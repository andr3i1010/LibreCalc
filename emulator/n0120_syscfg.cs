// SPDX-License-Identifier: GPL-3.0-only

using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using Antmicro.Renode.Core;
using Antmicro.Renode.Peripherals.Bus;

namespace Antmicro.Renode.Peripherals
{
    // STM32H7B0 SYSCFG subset: EXTI source selection and CMPCR readiness.
    [GPIO(NumberOfOutputs = Pins)]
    public sealed class N0120Syscfg : IDoubleWordPeripheral, IKnownSize, INumberedGPIOOutput, ILocalGPIOReceiver
    {
        public N0120Syscfg()
        {
            var connections = new Dictionary<int, IGPIO>();
            for(var pin = 0; pin < Pins; pin++)
            {
                connections.Add(pin, new GPIO());
            }
            Connections = new ReadOnlyDictionary<int, IGPIO>(connections);
        }

        public IGPIOReceiver GetLocalReceiver(int port)
        {
            if(port < 0 || port >= Ports)
            {
                throw new ArgumentOutOfRangeException(nameof(port));
            }
            if(receivers[port] == null)
            {
                receivers[port] = new PortReceiver(this, port);
            }
            return receivers[port];
        }

        public uint ReadDoubleWord(long offset)
        {
            if(offset < 0 || offset >= Size)
            {
                return 0;
            }
            var value = registers[offset >> 2];
            return offset == CompensationControl && (value & CompensationEnable) != 0
                ? value | CompensationReady
                : value & ~CompensationReady;
        }

        public void WriteDoubleWord(long offset, uint value)
        {
            if(offset < 0 || offset >= Size)
            {
                return;
            }
            if(offset == CompensationControl)
            {
                registers[offset >> 2] = value & ~CompensationReady;
                return;
            }

            registers[offset >> 2] = value;
            if(offset < ExternalInterruptConfiguration1 || offset > ExternalInterruptConfiguration4)
            {
                return;
            }
            var firstPin = (int)(offset - ExternalInterruptConfiguration1);
            for(var pin = firstPin; pin < firstPin + 4; pin++)
            {
                UpdatePin(pin);
            }
        }

        public void Reset()
        {
            Array.Clear(registers, 0, registers.Length);
            foreach(var connection in Connections.Values)
            {
                connection.Unset();
            }
            for(var pin = 0; pin < Pins; pin++)
            {
                UpdatePin(pin);
            }
        }

        public IReadOnlyDictionary<int, IGPIO> Connections { get; }

        public long Size => 0x400;

        private void UpdatePin(int pin)
        {
            Connections[pin].Set(levels[SelectedPort(pin), pin]);
        }

        private int SelectedPort(int pin)
        {
            return (int)((registers[ExternalInterruptConfiguration1 / 4 + pin / 4] >> (pin % 4 * 4)) & 0xF);
        }

        private readonly uint[] registers = new uint[0x400 / 4];
        private readonly bool[,] levels = new bool[Ports, Pins];
        private readonly PortReceiver[] receivers = new PortReceiver[Ports];

        private const int Ports = 11;
        private const int Pins = 16;
        private const long ExternalInterruptConfiguration1 = 0x08;
        private const long ExternalInterruptConfiguration4 = 0x14;
        private const long CompensationControl = 0x20;
        private const uint CompensationEnable = 1 << 0;
        private const uint CompensationReady = 1 << 8;

        private sealed class PortReceiver : IGPIOReceiver
        {
            public PortReceiver(N0120Syscfg parent, int port)
            {
                this.parent = parent;
                this.port = port;
            }

            public void OnGPIO(int pin, bool value)
            {
                if(pin < 0 || pin >= Pins)
                {
                    throw new ArgumentOutOfRangeException(nameof(pin));
                }
                parent.levels[port, pin] = value;
                if(parent.SelectedPort(pin) == port)
                {
                    parent.Connections[pin].Set(value);
                }
            }

            public void Reset()
            {
            }

            private readonly N0120Syscfg parent;
            private readonly int port;
        }
    }
}
