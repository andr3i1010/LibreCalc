// SPDX-License-Identifier: GPL-3.0-only

using System;
using System.Linq;
using Antmicro.Renode.Core;
using Antmicro.Renode.Peripherals.Bus;
using Antmicro.Renode.Peripherals.CPU;

namespace Antmicro.Renode.Peripherals
{
    // N0120 PWR subset used by the stock boot path. WKUP1 is PA0 and signals
    // the Cortex-M event register only when its hardware enable/polarity bits
    // select the observed input transition.
    [GPIO(NumberOfInputs = 1)]
    public sealed class N0120Power : IDoubleWordPeripheral, IKnownSize, IGPIOReceiver
    {
        public N0120Power(Machine machine)
        {
            this.machine = machine;
            Reset();
        }

        public void Reset()
        {
            Array.Clear(regs, 0, regs.Length);
            regs[0x00 >> 2] = 0xF000C100;
            regs[0x04 >> 2] = 0x00006000;
            regs[0x0C >> 2] = 0x0000000C;
            regs[0x18 >> 2] = 0x00002000;
            wakeup1Level = false;
            wakeupFlags = 0;
        }

        public uint ReadDoubleWord(long offset)
        {
            if(offset == 0x24)
            {
                return wakeupFlags;
            }
            return regs[offset >> 2];
        }

        public void WriteDoubleWord(long offset, uint value)
        {
            if(offset == 0x20)
            {
                wakeupFlags &= ~(value & 0x3F);
                return;
            }
            if(offset == 0x24)
            {
                return;
            }

            regs[offset >> 2] = value;
            if(offset == 0x0C)
            {
                regs[offset >> 2] &= ~0x04000000u;
                if((value & 0x01000000) != 0)
                {
                    regs[offset >> 2] |= 0x04000000;
                }
            }
            else if(offset == 0x18)
            {
                regs[offset >> 2] |= 0x00002000;
            }
            else if(offset == 0x28)
            {
                CheckWakeup1(false);
            }
        }

        public void OnGPIO(int number, bool value)
        {
            if(number != 0)
            {
                throw new ArgumentOutOfRangeException(nameof(number));
            }
            var wasActive = IsWakeup1Active(wakeup1Level);
            wakeup1Level = value;
            CheckWakeup1(wasActive);
        }

        public long Size => 0x400;

        private bool IsWakeup1Active(bool level)
        {
            return (regs[0x28 >> 2] & 0x100) == 0 ? level : !level;
        }

        private void CheckWakeup1(bool wasActive)
        {
            if((regs[0x28 >> 2] & 1) == 0 || wasActive || !IsWakeup1Active(wakeup1Level))
            {
                return;
            }
            wakeupFlags |= 1;
            machine.SystemBus.GetCPUs().OfType<Arm>().Single().SetEventFlag(true);
        }

        private readonly Machine machine;
        private readonly uint[] regs = new uint[256];
        private bool wakeup1Level;
        private uint wakeupFlags;
    }
}
