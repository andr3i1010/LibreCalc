// SPDX-License-Identifier: GPL-3.0-only

using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.IO;
using System.Net;
using System.Net.Sockets;
using System.Text;
using System.Threading;

using Antmicro.Renode.Core;
using Antmicro.Renode.Core.USB;
using Antmicro.Renode.Peripherals;
using Antmicro.Renode.Peripherals.Bus;

namespace Antmicro.Renode.Peripherals
{
    // STM32H725 OTG-HS device-mode subset used by the N0120's control-only
    // USB stack. Descriptors and DFU protocol remain entirely guest-driven.
    [GPIO(NumberOfOutputs = 2)]
    public sealed class N0120UsbOtgHs : IDoubleWordPeripheral, IKnownSize, INumberedGPIOOutput, IUSBDevice, IDisposable
    {
        public N0120UsbOtgHs(Machine machine)
        {
            this.machine = machine;
            Connections = new ReadOnlyDictionary<int, IGPIO>(new Dictionary<int, IGPIO> { [0] = new GPIO(), [1] = new GPIO() });
            USBCore = new USBDeviceCore(this, customSetupPacketHandler: HandleSetupPacket);
            Reset();
        }

        public USBDeviceCore USBCore { get; }

        public IReadOnlyDictionary<int, IGPIO> Connections { get; }

        public long Size => 0x2000;

        public bool CableConnected
        {
            get
            {
                lock(sync)
                {
                    return cableConnected;
                }
            }
            set
            {
                var stopServer = false;
                lock(sync)
                {
                    if(cableConnected == value)
                    {
                        return;
                    }
                    cableConnected = value;
                    if(value)
                    {
                        gotgctl |= BSessionValid;
                        MaybeSignalBusReset();
                    }
                    else
                    {
                        gotgctl &= ~BSessionValid;
                        gotgint |= SessionEndDetected;
                        CancelTransfer();
                        busResetSignaled = false;
                        hostRequested = false;
                        stopServer = usbIpServer != null;
                    }
                    Connections[1].Set(value);
                    UpdateInterrupt();
                }
                if(stopServer)
                {
                    StopUSBIP();
                }
            }
        }

        public bool IsDeviceReady
        {
            get
            {
                lock(sync)
                {
                    return IsDeviceReadyUnsafe();
                }
            }
        }

        public bool StartUSBIP()
        {
            TcpListener listener;
            Thread thread;
            lock(sync)
            {
                if(usbIpServer != null)
                {
                    return true;
                }
                if(!GuestReadyUnsafe())
                {
                    return false;
                }
                try
                {
                    listener = new TcpListener(IPAddress.Loopback, USBIPPort);
                    listener.Start();
                    usbIpServer = listener;
                    usbIpCancellation = new CancellationTokenSource();
                    thread = new Thread(ServeUSBIP) { IsBackground = true, Name = "LibreCalc USB/IP" };
                    usbIpThread = thread;
                }
                catch(Exception)
                {
                    hostError = true;
                    return false;
                }
            }
            thread.Start();
            return true;
        }

        public void StopUSBIP()
        {
            TcpListener listener;
            TcpClient client;
            Thread thread;
            CancellationTokenSource cancellation;
            lock(sync)
            {
                listener = usbIpServer;
                client = usbIpClient;
                thread = usbIpThread;
                cancellation = usbIpCancellation;
                usbIpServer = null;
                usbIpClient = null;
                usbIpThread = null;
                usbIpCancellation = null;
                CancelTransfer();
            }
            if(listener == null)
            {
                return;
            }
            cancellation.Cancel();
            client?.Close();
            listener.Stop();
            if(thread != Thread.CurrentThread)
            {
                thread.Join(1000);
            }
            cancellation.Dispose();
        }

        public void Dispose()
        {
            StopUSBIP();
        }

        public void Reset()
        {
            lock(sync)
            {
                gotgctl = DeviceMode;
                gotgint = 0;
                gahbcfg = 0;
                gusbcfg = 0x0a00;
                gintsts = 0;
                gintmsk = 0;
                grxfsiz = 0;
                dieptxf0 = 0;
                gccfg = 0;
                dcfg = 0x02200000;
                dctl = 0;
                diepmsk = 0;
                doepmsk = 0;
                daintmsk = 0;
                diepctl0 = 0;
                diepint0 = 0;
                dieptsiz0 = 0;
                doepctl0 = 0x8000;
                doepint0 = 0;
                doeptsiz0 = 0;
                pcgcctl = 0;
                hostRequested = false;
                hostError = false;
                rxStatuses.Clear();
                rxFifo.Clear();
                txFifo.Clear();
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
                case Gotgctl: return gotgctl | (cableConnected ? BSessionValid : 0) | DeviceMode;
                case Gotgint: return gotgint;
                case Gahbcfg: return gahbcfg;
                case Gusbcfg: return gusbcfg;
                case Grstctl: return AhbIdle;
                case Gintsts: return CurrentInterruptStatus();
                case Gintmsk: return gintmsk;
                case Grxstsr: return rxStatuses.Count == 0 ? 0u : rxStatuses.Peek();
                case Grxstsp: return PopReceiveStatus();
                case Grxfsiz: return grxfsiz;
                case Dieptxf0: return dieptxf0;
                case Gccfg: return gccfg;
                case CoreId: return 0x4f54280a;
                case Dcfg: return dcfg;
                case Dctl: return dctl;
                case Dsts: return 0x10 | (cableConnected ? FullSpeed : 0);
                case Diepmsk: return diepmsk;
                case Doepmsk: return doepmsk;
                case Daint: return CurrentDaint();
                case Daintmsk: return daintmsk;
                case Diepctl0: return diepctl0;
                case Diepint0: return diepint0;
                case Dieptsiz0: return dieptsiz0;
                case Dtxfsts0: return 0x100;
                case Doepctl0: return doepctl0;
                case Doepint0: return doepint0;
                case Doeptsiz0: return doeptsiz0;
                case Pcgcctl: return pcgcctl;
                case HostStatus:
                    return (cableConnected ? VirtualCable : 0)
                        | (GuestReadyUnsafe() ? HostRequested : 0)
                        | (usbIpServer != null ? HostExported : 0)
                        | (hostError ? HostError : 0);
                case Fifo0: return ReadFifo();
                default: return 0;
                }
            }
        }

        public void WriteDoubleWord(long offset, uint value)
        {
            var stopServer = false;
            var startServer = false;
            lock(sync)
            {
                switch(offset)
                {
                case Gotgint:
                    gotgint &= ~value;
                    break;
                case Gahbcfg:
                    gahbcfg = value;
                    MaybeSignalBusReset();
                    break;
                case Gusbcfg:
                    gusbcfg = value;
                    break;
                case Grstctl:
                    if((value & ReceiveFifoFlush) != 0)
                    {
                        rxStatuses.Clear();
                        rxFifo.Clear();
                    }
                    if((value & TransmitFifoFlush) != 0)
                    {
                        txFifo.Clear();
                        dieptsiz0 = 0;
                    }
                    if((value & CoreSoftReset) != 0)
                    {
                        USBCore.Reset();
                    }
                    break;
                case Gintsts:
                    gintsts &= ~(value & WritableGlobalInterrupts);
                    startServer = hostRequested && usbIpServer == null && GuestReadyUnsafe();
                    break;
                case Gintmsk:
                    gintmsk = value;
                    MaybeSignalBusReset();
                    break;
                case Grxfsiz:
                    grxfsiz = value;
                    break;
                case Dieptxf0:
                    dieptxf0 = value;
                    break;
                case Gccfg:
                    gccfg = value;
                    MaybeSignalBusReset();
                    break;
                case Dcfg:
                    dcfg = value;
                    break;
                case Dctl:
                    dctl = value & (SoftDisconnect | PowerOnProgramDone | RemoteWakeup);
                    if((dctl & SoftDisconnect) != 0)
                    {
                        gotgint |= SessionEndDetected;
                        CancelTransfer();
                        busResetSignaled = false;
                        hostRequested = false;
                        stopServer = usbIpServer != null;
                    }
                    else
                    {
                        MaybeSignalBusReset();
                    }
                    break;
                case Diepmsk:
                    diepmsk = value;
                    break;
                case Doepmsk:
                    doepmsk = value;
                    break;
                case Daintmsk:
                    daintmsk = value;
                    break;
                case Diepctl0:
                    WriteInControl(value);
                    startServer = hostRequested && usbIpServer == null && GuestReadyUnsafe();
                    break;
                case Diepint0:
                    diepint0 &= ~value;
                    if((value & TransferComplete) != 0 && inTransferCompletePending)
                    {
                        inTransferCompletePending = false;
                        diepint0 |= TransferComplete;
                    }
                    else if((value & TransferComplete) != 0 && awaitingInAcknowledge)
                    {
                        CompleteTransfer();
                    }
                    break;
                case Dieptsiz0:
                    dieptsiz0 = value;
                    break;
                case Doepctl0:
                    WriteOutControl(value);
                    startServer = hostRequested && usbIpServer == null && GuestReadyUnsafe();
                    break;
                case Doepint0:
                    doepint0 &= ~value;
                    break;
                case Doeptsiz0:
                    doeptsiz0 = value;
                    break;
                case Pcgcctl:
                    pcgcctl = value;
                    break;
                case HostControl:
                    hostError = false;
                    hostRequested = (value & HostRequested) != 0;
                    if((value & VirtualCable) == 0)
                    {
                        cableConnected = false;
                        gotgctl &= ~BSessionValid;
                        gotgint |= SessionEndDetected;
                        CancelTransfer();
                        busResetSignaled = false;
                        hostRequested = false;
                        stopServer = usbIpServer != null;
                    }
                    else
                    {
                        cableConnected = true;
                        gotgctl |= BSessionValid;
                        MaybeSignalBusReset();
                        startServer = hostRequested && usbIpServer == null && GuestReadyUnsafe();
                    }
                    Connections[1].Set(cableConnected);
                    if(!hostRequested)
                    {
                        stopServer |= usbIpServer != null;
                    }
                    break;
                case Fifo0:
                    WriteFifo(value);
                    break;
                }
                UpdateInterrupt();
            }
            if(stopServer)
            {
                StopUSBIP();
            }
            if(startServer)
            {
                StartUSBIP();
            }
        }

        private void ServeUSBIP()
        {
            TcpListener listener;
            CancellationToken token;
            lock(sync)
            {
                listener = usbIpServer;
                token = usbIpCancellation.Token;
            }
            while(!token.IsCancellationRequested)
            {
                TcpClient client;
                try
                {
                    client = listener.AcceptTcpClient();
                }
                catch(SocketException)
                {
                    if(token.IsCancellationRequested)
                    {
                        return;
                    }
                    continue;
                }
                catch(ObjectDisposedException)
                {
                    return;
                }

                lock(sync)
                {
                    usbIpClient = client;
                }
                try
                {
                    HandleUSBIPConnection(client.GetStream(), token);
                }
                catch(IOException)
                {
                }
                catch(ObjectDisposedException)
                {
                }
                finally
                {
                    client.Close();
                    lock(sync)
                    {
                        if(usbIpClient == client)
                        {
                            usbIpClient = null;
                        }
                    }
                }
            }
        }

        private void HandleUSBIPConnection(Stream stream, CancellationToken token)
        {
            while(!token.IsCancellationRequested)
            {
                var header = ReadExactly(stream, USBIPHeaderLength);
                if(header == null || ReadUInt16(header, 0) != USBIPProtocolVersion)
                {
                    return;
                }
                switch(ReadUInt16(header, 2))
                {
                case USBIPListDevices:
                {
                    byte[] device;
                    List<byte[]> interfaces;
                    if(!TryReadDeviceInfo(out device, out interfaces))
                    {
                        WriteUSBIPHeader(stream, USBIPListDevicesReply, 1);
                        return;
                    }
                    WriteUSBIPHeader(stream, USBIPListDevicesReply, 0);
                    WriteUInt32(stream, 1);
                    stream.Write(device, 0, device.Length);
                    foreach(var iface in interfaces)
                    {
                        stream.Write(iface, 0, iface.Length);
                    }
                    break;
                }
                case USBIPAttachDevice:
                {
                    var busId = ReadExactly(stream, USBIPBusIdLength);
                    byte[] device = null;
                    List<byte[]> interfaces = null;
                    var attached = busId != null && Encoding.ASCII.GetString(busId).TrimEnd('\0') == USBIPBusId
                        && TryReadDeviceInfo(out device, out interfaces);
                    WriteUSBIPHeader(stream, USBIPAttachDeviceReply, attached ? 0u : 1u);
                    if(!attached)
                    {
                        return;
                    }
                    stream.Write(device, 0, device.Length);
                    HandleUSBIPUrbs(stream, token);
                    return;
                }
                default:
                    return;
                }
            }
        }

        private bool TryReadDeviceInfo(out byte[] deviceInfo, out List<byte[]> interfaces)
        {
            deviceInfo = null;
            interfaces = new List<byte[]>();
            var descriptor = ReadDescriptor(USBDeviceDescriptor, 0, USBDeviceDescriptorLength);
            if(descriptor == null || descriptor.Length < USBDeviceDescriptorLength)
            {
                return false;
            }

            var configurationValue = (byte)0;
            for(byte configuration = 0; configuration < descriptor[17]; configuration++)
            {
                var header = ReadDescriptor(USBConfigurationDescriptor, configuration, USBConfigurationDescriptorLength);
                if(header == null || header.Length < USBConfigurationDescriptorLength)
                {
                    return false;
                }
                if(configuration == 0)
                {
                    configurationValue = header[5];
                }
                var totalLength = ReadUInt16LittleEndian(header, 2);
                var contents = ReadDescriptor(USBConfigurationDescriptor, configuration, totalLength);
                if(contents == null || contents.Length < totalLength)
                {
                    return false;
                }
                for(var offset = 0; offset + 1 < contents.Length; offset += contents[offset])
                {
                    if(contents[offset] == 0)
                    {
                        return false;
                    }
                    if(contents[offset + 1] == USBInterfaceDescriptor && contents[offset] >= USBInterfaceDescriptorLength
                        && contents[offset + 3] == 0)
                    {
                        interfaces.Add(new[] { contents[offset + 5], contents[offset + 6], contents[offset + 7], (byte)0 });
                    }
                }
            }

            deviceInfo = new byte[USBIPDeviceInfoLength];
            CopyText(deviceInfo, 0, USBIPPathLength, "/librecalc/virtual/1-0");
            CopyText(deviceInfo, USBIPPathLength, USBIPBusIdLength, USBIPBusId);
            WriteUInt32(deviceInfo, 288, 1);
            WriteUInt32(deviceInfo, 292, 0);
            WriteUInt32(deviceInfo, 296, USBIPFullSpeed);
            WriteUInt16(deviceInfo, 300, ReadUInt16LittleEndian(descriptor, 8));
            WriteUInt16(deviceInfo, 302, ReadUInt16LittleEndian(descriptor, 10));
            WriteUInt16(deviceInfo, 304, ReadUInt16LittleEndian(descriptor, 12));
            deviceInfo[306] = descriptor[4];
            deviceInfo[307] = descriptor[5];
            deviceInfo[308] = descriptor[6];
            deviceInfo[309] = configurationValue;
            deviceInfo[310] = descriptor[17];
            deviceInfo[311] = (byte)interfaces.Count;
            return true;
        }

        private byte[] ReadDescriptor(byte type, byte index, ushort length)
        {
            return RequestSetup(new SetupPacket {
                Direction = Direction.DeviceToHost,
                Type = PacketType.Standard,
                Recipient = PacketRecipient.Device,
                Request = GetDescriptor,
                Value = (ushort)((type << 8) | index),
                Count = length,
            });
        }

        private byte[] RequestSetup(SetupPacket packet, byte[] data = null)
        {
            byte[] response = null;
            using(var completed = new ManualResetEvent(false))
            {
                machine.LocalTimeSource.ExecuteInNearestSyncedState(_ =>
                    USBCore.HandleSetupPacket(packet, additionalData: data, resultCallback: result =>
                    {
                        response = result;
                        completed.Set();
                    }));
                if(completed.WaitOne(USBIPRequestTimeout))
                {
                    return response ?? Array.Empty<byte>();
                }
                machine.LocalTimeSource.ExecuteInNearestSyncedState(_ =>
                {
                    lock(sync)
                    {
                        CancelTransfer();
                    }
                });
                return null;
            }
        }

        private void HandleUSBIPUrbs(Stream stream, CancellationToken token)
        {
            while(!token.IsCancellationRequested)
            {
                var request = ReadExactly(stream, USBIPUrbHeaderLength);
                if(request == null)
                {
                    return;
                }
                var command = ReadUInt32(request, 0);
                if(command == USBIPSubmit)
                {
                    var direction = ReadUInt32(request, 12);
                    var endpoint = ReadUInt32(request, 16);
                    var length = unchecked((int)ReadUInt32(request, 24));
                    if(length < 0 || length > USBIPMaximumTransfer)
                    {
                        return;
                    }
                    var payload = direction == USBIPOut && length != 0 ? ReadExactly(stream, length) : Array.Empty<byte>();
                    if(payload == null)
                    {
                        return;
                    }

                    var status = 0;
                    byte[] response = Array.Empty<byte>();
                    if(endpoint != 0)
                    {
                        status = USBIPStall;
                    }
                    else
                    {
                        var setup = new SetupPacket {
                            Direction = (request[40] & 0x80) != 0 ? Direction.DeviceToHost : Direction.HostToDevice,
                            Type = (PacketType)((request[40] >> 5) & 3),
                            Recipient = (PacketRecipient)(request[40] & 0x1f),
                            Request = request[41],
                            Value = ReadUInt16LittleEndian(request, 42),
                            Index = ReadUInt16LittleEndian(request, 44),
                            Count = ReadUInt16LittleEndian(request, 46),
                        };
                        response = RequestSetup(setup, payload);
                        if(ReferenceEquals(response, stalledTransfer))
                        {
                            status = USBIPStall;
                            response = Array.Empty<byte>();
                        }
                        else if(response == null)
                        {
                            status = USBIPTimeout;
                            response = Array.Empty<byte>();
                        }
                    }
                    WriteUSBIPUrbReply(stream, USBIPReturnSubmit, request, status,
                        direction == USBIPIn ? response.Length : payload.Length,
                        direction == USBIPIn ? response : Array.Empty<byte>());
                }
                else if(command == USBIPUnlink)
                {
                    WriteUSBIPUrbReply(stream, USBIPReturnUnlink, request, 0, 0, Array.Empty<byte>());
                }
                else
                {
                    return;
                }
            }
        }

        private static void WriteUSBIPUrbReply(Stream stream, uint command, byte[] request, int status, int actualLength, byte[] data)
        {
            var reply = new byte[USBIPUrbHeaderLength];
            WriteUInt32(reply, 0, command);
            Buffer.BlockCopy(request, 4, reply, 4, 16);
            WriteUInt32(reply, 20, unchecked((uint)status));
            WriteUInt32(reply, 24, (uint)actualLength);
            Buffer.BlockCopy(request, 40, reply, 40, 8);
            stream.Write(reply, 0, reply.Length);
            if(data.Length != 0)
            {
                stream.Write(data, 0, data.Length);
            }
        }

        private static byte[] ReadExactly(Stream stream, int length)
        {
            var data = new byte[length];
            var offset = 0;
            while(offset < length)
            {
                var count = stream.Read(data, offset, length - offset);
                if(count == 0)
                {
                    return null;
                }
                offset += count;
            }
            return data;
        }

        private static void WriteUSBIPHeader(Stream stream, ushort command, uint status)
        {
            var header = new byte[USBIPHeaderLength];
            WriteUInt16(header, 0, USBIPProtocolVersion);
            WriteUInt16(header, 2, command);
            WriteUInt32(header, 4, status);
            stream.Write(header, 0, header.Length);
        }

        private static void CopyText(byte[] destination, int offset, int length, string text)
        {
            var bytes = Encoding.ASCII.GetBytes(text);
            Array.Copy(bytes, 0, destination, offset, Math.Min(bytes.Length, length - 1));
        }

        private static ushort ReadUInt16(byte[] value, int offset)
        {
            return (ushort)((value[offset] << 8) | value[offset + 1]);
        }

        private static ushort ReadUInt16LittleEndian(byte[] value, int offset)
        {
            return (ushort)(value[offset] | (value[offset + 1] << 8));
        }

        private static uint ReadUInt32(byte[] value, int offset)
        {
            return ((uint)value[offset] << 24) | ((uint)value[offset + 1] << 16)
                | ((uint)value[offset + 2] << 8) | value[offset + 3];
        }

        private static void WriteUInt16(byte[] destination, int offset, ushort value)
        {
            destination[offset] = (byte)(value >> 8);
            destination[offset + 1] = (byte)value;
        }

        private static void WriteUInt32(Stream destination, uint value)
        {
            var data = new byte[4];
            WriteUInt32(data, 0, value);
            destination.Write(data, 0, data.Length);
        }

        private static void WriteUInt32(byte[] destination, int offset, uint value)
        {
            destination[offset] = (byte)(value >> 24);
            destination[offset + 1] = (byte)(value >> 16);
            destination[offset + 2] = (byte)(value >> 8);
            destination[offset + 3] = (byte)value;
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
                if(packet.Type == PacketType.Standard && packet.Request == SetConfiguration && USBCore.Address == 0)
                {
                    deferredPacket = packet;
                    deferredData = data;
                    hasDeferredPacket = true;
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
            outDataArmed = false;
            awaitingInAcknowledge = false;
            awaitingStatusOut = false;
            statusOutQueued = false;
            txFifo.Clear();
            reply.Clear();
            diepctl0 &= ~Stall;
            QueueReceive(Encode(packet), SetupReceived);
            QueueReceive(Array.Empty<byte>(), SetupCompleted);
        }

        private void WriteInControl(uint value)
        {
            var sticky = diepctl0 & (EndpointActive | NAKStatus | Stall);
            diepctl0 = sticky | (value & (MaximumPacketSize | EndpointActive | Stall | TxFifoNumber));
            if((value & SetNak) != 0)
            {
                diepctl0 |= NAKStatus;
                diepint0 |= InNakEffective;
            }
            if((value & ClearNak) != 0)
            {
                diepctl0 &= ~NAKStatus;
            }
            if((value & EndpointEnable) != 0)
            {
                diepctl0 |= EndpointActive;
            }
            if((value & EndpointDisable) != 0)
            {
                diepctl0 &= ~EndpointActive;
                diepint0 |= EndpointDisabled;
            }
            if((diepctl0 & Stall) != 0)
            {
                var callback = pendingCallback;
                CancelTransfer();
                callback?.Invoke(stalledTransfer);
                return;
            }
            if((value & ClearNak) != 0 && (dieptsiz0 & TransferSizeMask) == 0 && pendingCallback != null)
            {
                FinishInPacket();
            }
        }

        private void WriteOutControl(uint value)
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
                outDataArmed = true;
                QueueNextOutPacket();
                if(awaitingStatusOut && !statusOutQueued)
                {
                    QueueStatusOut();
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
                callback?.Invoke(stalledTransfer);
            }
        }

        private void WriteFifo(uint value)
        {
            txFifo.Add((byte)value);
            txFifo.Add((byte)(value >> 8));
            txFifo.Add((byte)(value >> 16));
            txFifo.Add((byte)(value >> 24));
            var count = (int)(dieptsiz0 & TransferSizeMask);
            if(count > 0 && txFifo.Count >= ((count + 3) & ~3))
            {
                FinishInPacket();
            }
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
            dieptsiz0 &= ~TransferSizeAndPacketCountMask;
            if((diepint0 & TransferComplete) != 0)
            {
                inTransferCompletePending = true;
            }
            else
            {
                diepint0 |= TransferComplete;
            }

            if(pendingPacket.Direction == Direction.DeviceToHost)
            {
                if(count < MaximumPacketSizeBytes || reply.Count >= pendingPacket.Count)
                {
                    awaitingStatusOut = true;
                }
            }
            else
            {
                awaitingInAcknowledge = true;
            }
        }

        private void QueueNextOutPacket()
        {
            if(!outDataArmed || pendingCallback == null || pendingPacket.Direction != Direction.HostToDevice ||
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

        private void QueueStatusOut()
        {
            statusOutQueued = true;
            QueueReceive(Array.Empty<byte>(), OutReceived);
            QueueReceive(Array.Empty<byte>(), OutCompleted);
        }

        private void QueueReceive(byte[] data, uint status)
        {
            foreach(var b in data)
            {
                rxFifo.Enqueue(b);
            }
            rxStatuses.Enqueue((uint)(data.Length << 4) | (status << 17));
            UpdateInterrupt();
        }

        private uint PopReceiveStatus()
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
            if(packetStatus == OutReceived && statusOutQueued && ((status >> 4) & 0x7ff) == 0)
            {
                CompleteTransfer();
            }
            UpdateInterrupt();
            return status;
        }

        private uint ReadFifo()
        {
            uint value = 0;
            for(var i = 0; i < 4 && rxFifo.Count != 0; i++)
            {
                value |= (uint)rxFifo.Dequeue() << (8 * i);
            }
            return value;
        }

        private void CompleteTransfer()
        {
            if(pendingCallback == null)
            {
                return;
            }
            if(hasDeferredPacket)
            {
                USBCore.Address = SpoofedAddress;
                var packet = deferredPacket;
                var data = deferredData;
                hasDeferredPacket = false;
                deferredData = null;
                BeginSetup(packet, data);
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
            outDataArmed = false;
            awaitingInAcknowledge = false;
            inTransferCompletePending = false;
            awaitingStatusOut = false;
            statusOutQueued = false;
            hasDeferredPacket = false;
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

        private bool IsDeviceReadyUnsafe()
        {
            return cableConnected && (dctl & SoftDisconnect) == 0 && (gccfg & PowerDown) != 0
                && (gahbcfg & GlobalInterruptMask) != 0 && (gintmsk & EnumerationDone) != 0;
        }

        private bool GuestReadyUnsafe()
        {
            return IsDeviceReadyUnsafe() && busResetSignaled && (gintsts & EnumerationDone) == 0
                && (diepctl0 & EndpointActive) != 0 && (doepctl0 & EndpointActive) != 0;
        }

        private uint CurrentDaint()
        {
            uint value = 0;
            if(diepint0 != 0)
            {
                value |= 1;
            }
            if(doepint0 != 0)
            {
                value |= 1u << 16;
            }
            return value;
        }

        private uint CurrentInterruptStatus()
        {
            var value = gintsts;
            if(rxStatuses.Count != 0)
            {
                value |= ReceiveFifoLevel;
            }
            if((diepint0 & diepmsk) != 0 && (daintmsk & 1) != 0)
            {
                value |= InEndpointInterrupt;
            }
            if((doepint0 & doepmsk) != 0 && (daintmsk & (1u << 16)) != 0)
            {
                value |= OutEndpointInterrupt;
            }
            return value;
        }

        private void UpdateInterrupt()
        {
            Connections[0].Set((gahbcfg & GlobalInterruptMask) != 0 && (CurrentInterruptStatus() & gintmsk) != 0);
        }

        private static byte[] Encode(SetupPacket packet)
        {
            return new[] {
                (byte)((byte)packet.Recipient | ((byte)packet.Type << 5) | ((byte)packet.Direction << 7)),
                packet.Request,
                (byte)packet.Value,
                (byte)(packet.Value >> 8),
                (byte)packet.Index,
                (byte)(packet.Index >> 8),
                (byte)packet.Count,
                (byte)(packet.Count >> 8),
            };
        }

        private readonly Machine machine;
        private readonly object sync = new object();
        private readonly Queue<uint> rxStatuses = new Queue<uint>();
        private readonly Queue<byte> rxFifo = new Queue<byte>();
        private readonly List<byte> txFifo = new List<byte>();
        private readonly List<byte> reply = new List<byte>();
        private readonly byte[] stalledTransfer = new byte[0];

        private TcpListener usbIpServer;
        private TcpClient usbIpClient;
        private Thread usbIpThread;
        private CancellationTokenSource usbIpCancellation;
        private Action<byte[]> pendingCallback;
        private SetupPacket pendingPacket;
        private SetupPacket deferredPacket;
        private byte[] pendingOut = Array.Empty<byte>();
        private byte[] deferredData;
        private int pendingOutOffset;
        private bool cableConnected;
        private bool busResetSignaled;
        private bool outDataArmed;
        private bool awaitingInAcknowledge;
        private bool inTransferCompletePending;
        private bool awaitingStatusOut;
        private bool statusOutQueued;
        private bool hasDeferredPacket;
        private bool hostRequested;
        private bool hostError;

        private uint gotgctl;
        private uint gotgint;
        private uint gahbcfg;
        private uint gusbcfg;
        private uint gintsts;
        private uint gintmsk;
        private uint grxfsiz;
        private uint dieptxf0;
        private uint gccfg;
        private uint dcfg;
        private uint dctl;
        private uint diepmsk;
        private uint doepmsk;
        private uint daintmsk;
        private uint diepctl0;
        private uint diepint0;
        private uint dieptsiz0;
        private uint doepctl0;
        private uint doepint0;
        private uint doeptsiz0;
        private uint pcgcctl;

        private const int USBIPPort = 3240;
        private const int USBIPHeaderLength = 8;
        private const int USBIPBusIdLength = 32;
        private const int USBIPPathLength = 256;
        private const int USBIPDeviceInfoLength = 312;
        private const int USBIPUrbHeaderLength = 48;
        private const int USBIPMaximumTransfer = 1 << 20;
        private const int USBIPRequestTimeout = 5000;
        private const string USBIPBusId = "1-0";
        private const ushort USBIPProtocolVersion = 0x0111;
        private const ushort USBIPListDevices = 0x8005;
        private const ushort USBIPListDevicesReply = 0x0005;
        private const ushort USBIPAttachDevice = 0x8003;
        private const ushort USBIPAttachDeviceReply = 0x0003;
        private const uint USBIPSubmit = 0x00000001;
        private const uint USBIPUnlink = 0x00000002;
        private const uint USBIPReturnSubmit = 0x00000003;
        private const uint USBIPReturnUnlink = 0x00000004;
        private const uint USBIPOut = 0;
        private const uint USBIPIn = 1;
        private const uint USBIPFullSpeed = 2;
        private const int USBIPStall = -32;
        private const int USBIPTimeout = -110;
        private const byte USBDeviceDescriptor = 1;
        private const byte USBConfigurationDescriptor = 2;
        private const byte USBInterfaceDescriptor = 4;
        private const ushort USBDeviceDescriptorLength = 18;
        private const ushort USBConfigurationDescriptorLength = 9;
        private const byte USBInterfaceDescriptorLength = 9;
        private const byte SpoofedAddress = 1;
        private const byte GetDescriptor = 6;
        private const byte SetAddress = 5;
        private const byte SetConfiguration = 9;
        private const uint Gotgctl = 0x000;
        private const uint Gotgint = 0x004;
        private const uint Gahbcfg = 0x008;
        private const uint Gusbcfg = 0x00c;
        private const uint Grstctl = 0x010;
        private const uint Gintsts = 0x014;
        private const uint Gintmsk = 0x018;
        private const uint Grxstsr = 0x01c;
        private const uint Grxstsp = 0x020;
        private const uint Grxfsiz = 0x024;
        private const uint Dieptxf0 = 0x028;
        private const uint Gccfg = 0x038;
        private const uint CoreId = 0x03c;
        private const uint Dcfg = 0x800;
        private const uint Dctl = 0x804;
        private const uint Dsts = 0x808;
        private const uint Diepmsk = 0x810;
        private const uint Doepmsk = 0x814;
        private const uint Daint = 0x818;
        private const uint Daintmsk = 0x81c;
        private const uint Diepctl0 = 0x900;
        private const uint Diepint0 = 0x908;
        private const uint Dieptsiz0 = 0x910;
        private const uint Dtxfsts0 = 0x918;
        private const uint Doepctl0 = 0xb00;
        private const uint Doepint0 = 0xb08;
        private const uint Doeptsiz0 = 0xb10;
        private const uint Pcgcctl = 0xe00;
        private const uint HostStatus = 0xe04;
        private const uint HostControl = 0xe08;
        private const uint Fifo0 = 0x1000;

        private const uint GlobalInterruptMask = 1u << 0;
        private const uint ReceiveFifoLevel = 1u << 4;
        private const uint UsbReset = 1u << 12;
        private const uint EnumerationDone = 1u << 13;
        private const uint InEndpointInterrupt = 1u << 18;
        private const uint OutEndpointInterrupt = 1u << 19;
        private const uint WritableGlobalInterrupts = UsbReset | EnumerationDone;
        private const uint CoreSoftReset = 1u << 0;
        private const uint ReceiveFifoFlush = 1u << 4;
        private const uint TransmitFifoFlush = 1u << 5;
        private const uint AhbIdle = 1u << 31;
        private const uint DeviceMode = 1u << 16;
        private const uint BSessionValid = 1u << 19;
        private const uint SessionEndDetected = 1u << 2;
        private const uint PowerDown = 1u << 16;
        private const uint SoftDisconnect = 1u << 1;
        private const uint RemoteWakeup = 1u << 0;
        private const uint PowerOnProgramDone = 1u << 11;
        private const uint VirtualCable = 1u << 0;
        private const uint HostRequested = 1u << 1;
        private const uint HostExported = 1u << 2;
        private const uint HostError = 1u << 3;
        private const uint MaximumPacketSize = 0x7ff;
        private const uint EndpointActive = 1u << 15;
        private const uint NAKStatus = 1u << 17;
        private const uint Stall = 1u << 21;
        private const uint TxFifoNumber = 0xfu << 22;
        private const uint ClearNak = 1u << 26;
        private const uint SetNak = 1u << 27;
        private const uint EndpointDisable = 1u << 30;
        private const uint EndpointEnable = 1u << 31;
        private const uint TransferComplete = 1u << 0;
        private const uint EndpointDisabled = 1u << 1;
        private const uint InNakEffective = 1u << 6;
        private const uint TxFifoEmpty = 1u << 7;
        private const uint TransferSizeMask = 0x7f;
        private const uint TransferSizeAndPacketCountMask = 0x0018007f;
        private const uint FullSpeed = 3u << 1;
        private const int MaximumPacketSizeBytes = 64;
        private const uint SetupReceived = 6;
        private const uint SetupCompleted = 4;
        private const uint OutReceived = 2;
        private const uint OutCompleted = 3;
    }
}
