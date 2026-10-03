// SPDX-License-Identifier: GPL-3.0-only

using System;
using System.Collections.Generic;
using Antmicro.Renode.Core;
using Antmicro.Renode.Core.USB;
using Antmicro.Renode.Peripherals.Bus;
using Antmicro.Renode.Utilities.Packets;

namespace Antmicro.Renode.Peripherals
{
    // STM32H725 OTG-HS device-mode subset used by the N0120's control-only
    // USB stack. Descriptors and DFU protocol remain entirely guest-driven.
    [GPIO(NumberOfOutputs = 2)]
    public sealed class N0120UsbOtgHs : IDoubleWordPeripheral, IKnownSize, INumberedGPIOOutput, IUSBDevice
    {
        public N0120UsbOtgHs(Machine machine)
        {
            this.machine = machine;
            Connections = new Dictionary<int, IGPIO> { [0] = new GPIO(), [1] = new GPIO() };
            USBCore = new USBDeviceCore(this, customSetupPacketHandler: HandleSetupPacket);
            Reset();
        }

        public USBDeviceCore USBCore { get; }

        public IReadOnlyDictionary<int, IGPIO> Connections { get; }

        public long Size => 0x2000;

        public void Reset()
        {
            lock(sync)
            {
                registers.Clear();
                registers[Gusbcfg] = 0x0a00;
                registers[Dcfg] = 0x02200000;
                gotgint = gahbcfg = gintsts = gintmsk = gccfg = dctl = diepmsk = daintmsk =
                    diepctl0 = diepint0 = dieptsiz0 = 0;
                doepctl0 = 0x8000;
                rxStatuses.Clear();
                rxFifo.Clear();
                USBCore.Reset();
                CancelTransfer();
                busResetSignaled = false;
                Connections[1].Set(false);
                if(cableConnected)
                {
                    machine.ScheduleAction(Antmicro.Renode.Time.TimeInterval.FromMicroseconds(1), _ =>
                    {
                        lock(sync)
                        {
                            Connections[1].Set(cableConnected);
                        }
                    });
                }
                UpdateInterrupt();
            }
        }

        public uint ReadDoubleWord(long offset)
        {
            lock(sync)
            {
                switch(offset)
                {
                case 0x000: return (cableConnected ? 1u << 19 : 0) | 1u << 16; // GOTGCTL
                case Gotgint: return gotgint;
                case Gahbcfg: return gahbcfg;
                case Grstctl: return 1u << 31;
                case Gintsts: return CurrentInterruptStatus();
                case Gintmsk: return gintmsk;
                case 0x01c: return rxStatuses.Count == 0 ? 0u : rxStatuses.Peek(); // GRXSTSR
                case 0x020: // GRXSTSP
                {
                    if(rxStatuses.Count == 0)
                    {
                        return 0;
                    }
                    var status = rxStatuses.Dequeue();
                    var packetStatus = (status >> 17) & 0xf;
                    if((packetStatus == SetupCompleted || packetStatus == OutCompleted) && pendingOutOffset < pendingOut.Length)
                    {
                        QueueNextOutPacket();
                    }
                    if(packetStatus == OutReceived && statusStage == StatusStage.OutQueued && ((status >> 4) & 0x7ff) == 0)
                    {
                        CompleteTransfer();
                    }
                    UpdateInterrupt();
                    return status;
                }
                case Gccfg: return gccfg;
                case 0x03c: return 0x4f54280a; // GSNPSID
                case Dctl: return dctl;
                case 0x808: return 0x10 | (cableConnected ? 3u << 1 : 0); // DSTS
                case Diepmsk: return diepmsk;
                case 0x818: return diepint0 != 0 ? 1u : 0; // DAINT
                case Daintmsk: return daintmsk;
                case Diepctl0: return diepctl0;
                case Diepint0: return diepint0;
                case Dieptsiz0: return dieptsiz0;
                case 0x918: return 0x100; // DTXFSTS0
                case Doepctl0: return doepctl0;
                case 0xe04: // Emulator host status
                    return (cableConnected ? VirtualCable : 0)
                        | (IsDeviceReadyUnsafe() && busResetSignaled && (gintsts & EnumerationDone) == 0
                            && (diepctl0 & EndpointActive) != 0 && (doepctl0 & EndpointActive) != 0 ? 1u << 1 : 0);
                case Fifo0:
                {
                    uint value = 0;
                    for(var i = 0; i < 4 && rxFifo.Count != 0; i++)
                    {
                        value |= (uint)rxFifo.Dequeue() << (8 * i);
                    }
                    return value;
                }
                default: return registers.TryGetValue(offset, out var stored) ? stored : 0;
                }
            }
        }

        public void WriteDoubleWord(long offset, uint value)
        {
            lock(sync)
            {
                switch(offset)
                {
                case Gusbcfg:
                case 0x024: // GRXFSIZ
                case 0x028: // DIEPTXF0
                case Dcfg:
                case 0x814: // DOEPMSK
                case 0xb10: // DOEPTSIZ0
                case 0xe00: // PCGCCTL
                    registers[offset] = value;
                    break;
                case Gotgint:
                    gotgint &= ~value;
                    break;
                case Gahbcfg:
                    gahbcfg = value;
                    MaybeSignalBusReset();
                    break;
                case Grstctl:
                    if((value & 1u << 4) != 0)
                    {
                        rxStatuses.Clear();
                        rxFifo.Clear();
                    }
                    if((value & 1u << 5) != 0)
                    {
                        txFifo.Clear();
                        dieptsiz0 = 0;
                    }
                    if((value & 1) != 0)
                    {
                        USBCore.Reset();
                    }
                    break;
                case Gintsts:
                    gintsts &= ~(value & (UsbReset | EnumerationDone));
                    break;
                case Gintmsk:
                    gintmsk = value;
                    MaybeSignalBusReset();
                    break;
                case Gccfg:
                    gccfg = value;
                    MaybeSignalBusReset();
                    break;
                case Dctl:
                    dctl = value & (SoftDisconnect | 1u << 11 | 1);
                    if((dctl & SoftDisconnect) != 0)
                    {
                        gotgint |= SessionEndDetected;
                        CancelTransfer();
                        busResetSignaled = false;
                    }
                    else
                    {
                        MaybeSignalBusReset();
                    }
                    break;
                case Diepmsk:
                    diepmsk = value;
                    break;
                case Daintmsk:
                    daintmsk = value;
                    break;
                case Diepctl0:
                {
                    var sticky = diepctl0 & (EndpointActive | NAKStatus | Stall);
                    diepctl0 = sticky | (value & (MaximumPacketSize | EndpointActive | Stall | 0xfu << 22));
                    if((value & SetNak) != 0)
                    {
                        diepctl0 |= NAKStatus;
                        diepint0 |= 1u << 6;
                    }
                    if((value & ClearNak) != 0)
                    {
                        diepctl0 &= ~NAKStatus;
                    }
                    if((value & EndpointEnable) != 0)
                    {
                        diepctl0 |= EndpointActive;
                    }
                    if((value & 1u << 30) != 0)
                    {
                        diepctl0 &= ~EndpointActive;
                        diepint0 |= 1u << 1;
                    }
                    if((diepctl0 & Stall) != 0)
                    {
                        var callback = pendingCallback;
                        CancelTransfer();
                        callback?.Invoke(Array.Empty<byte>());
                    }
                    else if((value & ClearNak) != 0 && (dieptsiz0 & TransferSizeMask) == 0 && pendingCallback != null)
                    {
                        FinishInPacket();
                    }
                    break;
                }
                case Diepint0:
                    diepint0 &= ~value;
                    if((value & TransferComplete) != 0 && inTransferCompletePending)
                    {
                        inTransferCompletePending = false;
                        diepint0 |= TransferComplete;
                    }
                    else if((value & TransferComplete) != 0 && statusStage == StatusStage.In)
                    {
                        CompleteTransfer();
                    }
                    break;
                case Dieptsiz0:
                    dieptsiz0 = value;
                    break;
                case Doepctl0:
                {
                    var sticky = doepctl0 & (EndpointActive | NAKStatus | Stall);
                    doepctl0 = sticky | (value & (MaximumPacketSize | EndpointActive | Stall));
                    if((value & SetNak) != 0)
                    {
                        doepctl0 |= NAKStatus;
                    }
                    if((value & ClearNak) != 0)
                    {
                        doepctl0 &= ~NAKStatus;
                        QueueNextOutPacket();
                        if(statusStage == StatusStage.Out)
                        {
                            statusStage = StatusStage.OutQueued;
                            QueueReceive(Array.Empty<byte>(), OutReceived);
                            QueueReceive(Array.Empty<byte>(), OutCompleted);
                        }
                    }
                    if((value & EndpointEnable) != 0)
                    {
                        doepctl0 |= EndpointActive;
                    }
                    if((value & Stall) != 0)
                    {
                        var callback = pendingCallback;
                        CancelTransfer();
                        callback?.Invoke(Array.Empty<byte>());
                    }
                    break;
                }
                case 0xe08: // Emulator host control
                    cableConnected = (value & VirtualCable) != 0;
                    if(cableConnected)
                    {
                        MaybeSignalBusReset();
                    }
                    else
                    {
                        gotgint |= SessionEndDetected;
                        CancelTransfer();
                        busResetSignaled = false;
                    }
                    Connections[1].Set(cableConnected);
                    break;
                case Fifo0:
                    txFifo.Add((byte)value);
                    txFifo.Add((byte)(value >> 8));
                    txFifo.Add((byte)(value >> 16));
                    txFifo.Add((byte)(value >> 24));
                    var count = (int)(dieptsiz0 & TransferSizeMask);
                    if(count > 0 && txFifo.Count >= ((count + 3) & ~3))
                    {
                        FinishInPacket();
                    }
                    break;
                }
                UpdateInterrupt();
            }
        }

        private void HandleSetupPacket(SetupPacket packet, byte[] data, Action<byte[]> resultCallback)
        {
            lock(sync)
            {
                if(pendingCallback != null || !IsDeviceReadyUnsafe())
                {
                    resultCallback(Array.Empty<byte>());
                    return;
                }
                pendingCallback = resultCallback;
                if(packet.Type == PacketType.Standard && packet.Request == 9 && USBCore.Address == 0)
                {
                    deferredPacket = packet;
                    deferredData = data;
                    BeginSetup(new SetupPacket {
                        Direction = Direction.HostToDevice,
                        Type = PacketType.Standard,
                        Recipient = PacketRecipient.Device,
                        Request = SetAddress,
                        Value = SpoofedAddress,
                    }, null);
                }
                else
                {
                    BeginSetup(packet, data);
                }
            }
        }

        private void BeginSetup(SetupPacket packet, byte[] data)
        {
            pendingPacket = packet;
            pendingOut = data ?? Array.Empty<byte>();
            pendingOutOffset = 0;
            statusStage = StatusStage.None;
            txFifo.Clear();
            reply.Clear();
            diepctl0 &= ~Stall;
            doepctl0 |= NAKStatus;
            QueueReceive(Packet.Encode(packet), 6);
            QueueReceive(Array.Empty<byte>(), SetupCompleted);
        }

        private void FinishInPacket()
        {
            if(pendingCallback == null)
            {
                return;
            }
            var count = (int)(dieptsiz0 & TransferSizeMask);
            if(count > txFifo.Count)
            {
                return;
            }
            if(count > 0)
            {
                reply.AddRange(txFifo.GetRange(0, count));
            }
            txFifo.Clear();
            dieptsiz0 &= ~0x0018007fu;
            if((diepint0 & TransferComplete) != 0)
            {
                inTransferCompletePending = true;
            }
            else
            {
                diepint0 |= TransferComplete;
            }

            if(pendingPacket.Direction == Direction.DeviceToHost &&
               (count < MaximumPacketSizeBytes || reply.Count >= pendingPacket.Count))
            {
                statusStage = StatusStage.Out;
            }
            else if(pendingPacket.Direction == Direction.HostToDevice)
            {
                statusStage = StatusStage.In;
            }
        }

        private void QueueNextOutPacket()
        {
            if((doepctl0 & NAKStatus) != 0 || pendingCallback == null || pendingPacket.Direction != Direction.HostToDevice ||
               pendingOutOffset >= pendingOut.Length || rxStatuses.Count != 0)
            {
                return;
            }
            var count = Math.Min(MaximumPacketSizeBytes, pendingOut.Length - pendingOutOffset);
            var packet = new byte[count];
            Array.Copy(pendingOut, pendingOutOffset, packet, 0, count);
            pendingOutOffset += count;
            QueueReceive(packet, OutReceived);
            QueueReceive(Array.Empty<byte>(), OutCompleted);
        }

        private void QueueReceive(byte[] data, uint status)
        {
            foreach(var b in data) rxFifo.Enqueue(b);
            rxStatuses.Enqueue((uint)(data.Length << 4) | (status << 17));
            UpdateInterrupt();
        }

        private void CompleteTransfer()
        {
            if(pendingCallback == null)
            {
                return;
            }
            if(deferredPacket.HasValue)
            {
                USBCore.Address = SpoofedAddress;
                BeginSetup(deferredPacket.Value, deferredData);
                deferredPacket = null;
                deferredData = null;
                return;
            }
            if(pendingPacket.Type == PacketType.Standard && pendingPacket.Request == SetAddress)
            {
                USBCore.Address = (byte)pendingPacket.Value;
            }
            var callback = pendingCallback;
            var result = pendingPacket.Direction == Direction.DeviceToHost ? reply.ToArray() : Array.Empty<byte>();
            CancelTransfer();
            callback(result);
        }

        private void CancelTransfer()
        {
            pendingCallback = null;
            pendingOut = Array.Empty<byte>();
            pendingOutOffset = 0;
            inTransferCompletePending = false;
            statusStage = StatusStage.None;
            deferredPacket = null;
            deferredData = null;
            reply.Clear();
            txFifo.Clear();
        }

        private void MaybeSignalBusReset()
        {
            if(!IsDeviceReadyUnsafe() || busResetSignaled)
            {
                return;
            }
            busResetSignaled = true;
            USBCore.Reset();
            gintsts |= UsbReset | EnumerationDone;
        }

        private bool IsDeviceReadyUnsafe() => cableConnected && (dctl & SoftDisconnect) == 0
            && (gccfg & 1u << 16) != 0 && (gahbcfg & GlobalInterruptMask) != 0
            && (gintmsk & EnumerationDone) != 0;

        private uint CurrentInterruptStatus()
        {
            return gintsts
                | (rxStatuses.Count != 0 ? 1u << 4 : 0)
                | ((diepint0 & diepmsk) != 0 && (daintmsk & 1) != 0 ? 1u << 18 : 0);
        }

        private void UpdateInterrupt()
        {
            Connections[0].Set((gahbcfg & GlobalInterruptMask) != 0 && (CurrentInterruptStatus() & gintmsk) != 0);
        }

        private readonly Machine machine;
        private readonly Dictionary<long, uint> registers = new Dictionary<long, uint>();
        private readonly object sync = new object();
        private readonly Queue<uint> rxStatuses = new Queue<uint>();
        private readonly Queue<byte> rxFifo = new Queue<byte>();
        private readonly List<byte> txFifo = new List<byte>();
        private readonly List<byte> reply = new List<byte>();

        private Action<byte[]> pendingCallback;
        private SetupPacket pendingPacket;
        private SetupPacket? deferredPacket;
        private byte[] pendingOut = Array.Empty<byte>();
        private byte[] deferredData;
        private int pendingOutOffset;
        private bool cableConnected;
        private bool busResetSignaled;
        private bool inTransferCompletePending;
        private StatusStage statusStage;

        private uint gotgint;
        private uint gahbcfg;
        private uint gintsts;
        private uint gintmsk;
        private uint gccfg;
        private uint dctl;
        private uint diepmsk;
        private uint daintmsk;
        private uint diepctl0;
        private uint diepint0;
        private uint dieptsiz0;
        private uint doepctl0;

        private const byte SpoofedAddress = 1;
        private const byte SetAddress = 5;
        private const uint Gotgint = 0x004;
        private const uint Gahbcfg = 0x008;
        private const uint Gusbcfg = 0x00c;
        private const uint Grstctl = 0x010;
        private const uint Gintsts = 0x014;
        private const uint Gintmsk = 0x018;
        private const uint Gccfg = 0x038;
        private const uint Dcfg = 0x800;
        private const uint Dctl = 0x804;
        private const uint Diepmsk = 0x810;
        private const uint Daintmsk = 0x81c;
        private const uint Diepctl0 = 0x900;
        private const uint Diepint0 = 0x908;
        private const uint Dieptsiz0 = 0x910;
        private const uint Doepctl0 = 0xb00;
        private const uint Fifo0 = 0x1000;

        private const uint GlobalInterruptMask = 1u << 0;
        private const uint UsbReset = 1u << 12;
        private const uint EnumerationDone = 1u << 13;
        private const uint SessionEndDetected = 1u << 2;
        private const uint SoftDisconnect = 1u << 1;
        private const uint VirtualCable = 1u << 0;
        private const uint MaximumPacketSize = 0x7ff;
        private const uint EndpointActive = 1u << 15;
        private const uint NAKStatus = 1u << 17;
        private const uint Stall = 1u << 21;
        private const uint ClearNak = 1u << 26;
        private const uint SetNak = 1u << 27;
        private const uint EndpointEnable = 1u << 31;
        private const uint TransferComplete = 1u << 0;
        private const uint TransferSizeMask = 0x7f;
        private const int MaximumPacketSizeBytes = 64;
        private const uint SetupCompleted = 4;
        private const uint OutReceived = 2;
        private const uint OutCompleted = 3;

        private enum StatusStage { None, In, Out, OutQueued }
    }
}
