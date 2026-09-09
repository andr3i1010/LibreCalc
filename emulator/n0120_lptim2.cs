// SPDX-License-Identifier: GPL-3.0-only

using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using Antmicro.Renode.Core;
using Antmicro.Renode.Peripherals;
using Antmicro.Renode.Peripherals.Bus;
using Antmicro.Renode.Time;

namespace Antmicro.Renode.Peripherals
{
    // STM32H7B0 LPTIM2. The N0120 stock boot path runs it continuously with
    // interrupts disabled, so counter state is derived from virtual time until
    // firmware enables an interrupt that needs a scheduled edge.
    [GPIO(NumberOfOutputs = 1)]
    public sealed class N0120Lptim2 : IDoubleWordPeripheral, IKnownSize, INumberedGPIOOutput
    {
        public N0120Lptim2(Machine machine)
        {
            this.machine = machine;
            var connections = new Dictionary<int, IGPIO> { [0] = new GPIO() };
            Connections = new ReadOnlyDictionary<int, IGPIO>(connections);
            Reset();
        }

        public void Reset()
        {
            status = 0;
            interruptEnable = 0;
            configuration = 0;
            control = 0;
            compare = 0;
            autoReload = 1;
            counter = 0;
            running = false;
            fractionalTicks = 0;
            lastUpdateTicks = machine.ClockSource.CurrentValue.Ticks;
            scheduleGeneration++;
            Connections[0].Unset();
        }

        public uint ReadDoubleWord(long offset)
        {
            Synchronize();
            switch(offset)
            {
                case InterruptAndStatus:
                    return status;
                case InterruptEnable:
                    return interruptEnable;
                case Configuration:
                    return configuration;
                case Control:
                    return control;
                case Compare:
                    return compare;
                case AutoReload:
                    return autoReload;
                case Counter:
                    return counter;
                default:
                    return 0;
            }
        }

        public void WriteDoubleWord(long offset, uint value)
        {
            Synchronize();
            switch(offset)
            {
                case InterruptClear:
                    status &= ~value;
                    RescheduleInterrupt();
                    return;
                case InterruptEnable:
                    interruptEnable = value;
                    RescheduleInterrupt();
                    return;
                case Configuration:
                    configuration = value;
                    RescheduleInterrupt();
                    return;
                case Control:
                    SetControl(value);
                    return;
                case Compare:
                    compare = value & 0xffff;
                    status |= CompareRegisterUpdateOk;
                    RescheduleInterrupt();
                    return;
                case AutoReload:
                    autoReload = value & 0xffff;
                    counter = (uint)(counter % Period);
                    status |= AutoReloadRegisterUpdateOk;
                    RescheduleInterrupt();
                    return;
            }
        }

        public IReadOnlyDictionary<int, IGPIO> Connections { get; }

        public long Size => 0x400;

        private void SetControl(uint value)
        {
            if((value & Enable) == 0)
            {
                control = 0;
                running = false;
                fractionalTicks = 0;
                scheduleGeneration++;
                Connections[0].Unset();
                return;
            }

            control = Enable;
            if((value & (SingleStart | ContinuousStart)) != 0)
            {
                control |= value & (SingleStart | ContinuousStart);
                counter = 0;
                fractionalTicks = 0;
                running = true;
            }
            lastUpdateTicks = machine.ClockSource.CurrentValue.Ticks;
            RescheduleInterrupt();
        }

        private void Synchronize()
        {
            if(!running)
            {
                return;
            }

            var now = machine.ClockSource.CurrentValue.Ticks;
            var elapsed = now - lastUpdateTicks;
            if(elapsed == 0)
            {
                return;
            }

            var frequency = InputClockFrequency / (1UL << (int)((configuration >> 9) & 7));
            var wholeSeconds = elapsed / TimeInterval.TicksPerSecond;
            var partialSeconds = elapsed % TimeInterval.TicksPerSecond;
            var scaledPartial = partialSeconds * frequency + fractionalTicks;
            var ticks = wholeSeconds * frequency + scaledPartial / TimeInterval.TicksPerSecond;
            fractionalTicks = scaledPartial % TimeInterval.TicksPerSecond;
            lastUpdateTicks = now;
            if(ticks == 0)
            {
                return;
            }

            var previous = counter;
            var period = Period;
            if(compare <= autoReload && Crosses(previous, ticks, compare, period))
            {
                status |= CompareMatch;
            }
            if(ticks >= period - previous)
            {
                status |= AutoReloadMatch;
            }

            if((control & SingleStart) != 0 && (control & ContinuousStart) == 0 && ticks >= period - previous)
            {
                counter = autoReload;
                running = false;
                fractionalTicks = 0;
            }
            else
            {
                counter = (uint)((previous + ticks) % period);
            }
            RefreshInterrupt();
        }

        private void RescheduleInterrupt()
        {
            scheduleGeneration++;
            RefreshInterrupt();
            if(!running || (status & interruptEnable) != 0)
            {
                return;
            }

            var steps = ulong.MaxValue;
            var period = Period;
            if((interruptEnable & CompareMatch) != 0 && compare <= autoReload)
            {
                steps = Distance(counter, compare, period);
            }
            if((interruptEnable & AutoReloadMatch) != 0)
            {
                steps = Math.Min(steps, period - counter);
            }
            if(steps == ulong.MaxValue)
            {
                return;
            }

            var frequency = InputClockFrequency / (1UL << (int)((configuration >> 9) & 7));
            var requiredTicks = steps * TimeInterval.TicksPerSecond - fractionalTicks;
            var delay = (requiredTicks + frequency - 1) / frequency;
            var generation = scheduleGeneration;
            machine.ScheduleAction(TimeInterval.FromTicks(delay), _ =>
            {
                if(generation != scheduleGeneration)
                {
                    return;
                }
                Synchronize();
                RescheduleInterrupt();
            }, "LPTIM2 IRQ");
        }

        private void RefreshInterrupt()
        {
            Connections[0].Set((status & interruptEnable) != 0);
        }

        private static bool Crosses(ulong current, ulong ticks, ulong target, ulong period)
        {
            return ticks >= Distance(current, target, period);
        }

        private static ulong Distance(ulong current, ulong target, ulong period)
        {
            var distance = target >= current ? target - current : period - current + target;
            return distance == 0 ? period : distance;
        }

        private ulong Period => (ulong)autoReload + 1;

        private readonly Machine machine;
        private uint status;
        private uint interruptEnable;
        private uint configuration;
        private uint control;
        private uint compare;
        private uint autoReload;
        private uint counter;
        private bool running;
        private ulong fractionalTicks;
        private ulong lastUpdateTicks;
        private uint scheduleGeneration;

        private const ulong InputClockFrequency = 32000000;
        private const long InterruptAndStatus = 0x00;
        private const long InterruptClear = 0x04;
        private const long InterruptEnable = 0x08;
        private const long Configuration = 0x0c;
        private const long Control = 0x10;
        private const long Compare = 0x14;
        private const long AutoReload = 0x18;
        private const long Counter = 0x1c;
        private const uint CompareMatch = 1 << 0;
        private const uint AutoReloadMatch = 1 << 1;
        private const uint CompareRegisterUpdateOk = 1 << 3;
        private const uint AutoReloadRegisterUpdateOk = 1 << 4;
        private const uint Enable = 1 << 0;
        private const uint SingleStart = 1 << 1;
        private const uint ContinuousStart = 1 << 2;
    }
}
