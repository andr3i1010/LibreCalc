// SPDX-License-Identifier: GPL-3.0-only

using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.IO;
using System.Text;
using Antmicro.Renode.Core;
using Antmicro.Renode.Peripherals;
using Antmicro.Renode.Peripherals.Bus;
using Antmicro.Renode.Time;

namespace Antmicro.Renode.Peripherals
{
    // ST7789V-compatible N0120 LCD on the FMC command/data windows. TE is PB1.
    [GPIO(NumberOfOutputs = 1)]
    public sealed class N0120Lcd : IWordPeripheral, IKnownSize, INumberedGPIOOutput
    {
        public N0120Lcd(Machine machine)
        {
            this.machine = machine;
            var connections = new Dictionary<int, IGPIO> { [0] = new GPIO() };
            Connections = new ReadOnlyDictionary<int, IGPIO>(connections);

            var root = Environment.GetEnvironmentVariable("LIBRECALC_ROOT");
            var state = root == null ? "/tmp" : Path.Combine(root, "emulator", "state");
            Directory.CreateDirectory(state);
            ppmPath = Path.Combine(state, "lcd.ppm");
            ppmTemporaryPath = ppmPath + ".tmp";
            busLog = new StreamWriter(Path.Combine(state, "lcd-bus.log"), false);
            busLog.WriteLine("# N0120 LCD bus capture");

            Reset();
        }

        public void Reset()
        {
            ResetController();
            Array.Clear(framebuffer, 0, framebuffer.Length);
            if(File.Exists(ppmPath))
            {
                File.Delete(ppmPath);
            }
        }

        public ushort ReadWord(long offset)
        {
            if(offset != DataOffset)
            {
                return 0;
            }
            if(command == ReadDisplayId)
            {
                return displayId[readIndex++ % displayId.Length];
            }
            if(command == MemoryRead)
            {
                if(readIndex++ == 0)
                {
                    return 0;
                }

                var displayX = x;
                var displayY = y;
                if((madctl & MemoryVerticalAddressing) == 0)
                {
                    displayX = Width - 1 - y;
                    displayY = x;
                }
                var pixel = 3 * (displayY * Width + displayX);
                var red = (framebuffer[pixel] * 31 + 127) / 255;
                var green = (framebuffer[pixel + 1] * 63 + 127) / 255;
                var blue = (framebuffer[pixel + 2] * 31 + 127) / 255;
                AdvanceCursor();
                return (ushort)(red << 11 | green << 5 | blue);
            }
            return 0;
        }

        public void WriteWord(long offset, ushort value)
        {
            if(offset == CommandOffset)
            {
                command = (byte)value;
                parameters = 0;
                readIndex = 0;
                busLog.WriteLine("C 2 0x{0:X4}", value & 0xFFFF);
                if(command == SoftwareReset)
                {
                    ResetController();
                }
                else if(command == SleepIn)
                {
                    sleeping = true;
                    UpdateTearingEffect();
                }
                else if(command == SleepOut)
                {
                    sleeping = false;
                    UpdateTearingEffect();
                }
                else if(command == TearingEffectOff)
                {
                    tearingEffectEnabled = false;
                    if(framebufferDirty)
                    {
                        PublishFrame();
                    }
                    UpdateTearingEffect();
                }
                else if(command == MemoryWrite)
                {
                    x = xStart;
                    y = yStart;
                }
                else if(command == MemoryRead)
                {
                    x = xStart;
                    y = yStart;
                }
                return;
            }
            if(offset != DataOffset)
            {
                return;
            }

            var data = (ushort)value;
            if(command == ColumnAddressSet || command == RowAddressSet)
            {
                windowParameters[parameters++] = (byte)data;
                busLog.WriteLine("D 0x{0:X4}", data);
                if(parameters == windowParameters.Length)
                {
                    var first = windowParameters[0] << 8 | windowParameters[1];
                    var last = windowParameters[2] << 8 | windowParameters[3];
                    if(command == ColumnAddressSet)
                    {
                        xStart = first;
                        xEnd = last;
                    }
                    else
                    {
                        yStart = first;
                        yEnd = last;
                    }
                    busLog.WriteLine("W {0} {1}..{2}", command == ColumnAddressSet ? "X" : "Y", first, last);
                    parameters = 0;
                }
            }
            else if(command == MemoryAccessControl)
            {
                madctl = (byte)data;
                busLog.WriteLine("D MADCTL 0x{0:X2}", madctl);
            }
            else if(command == TearingEffectOn)
            {
                tearingEffectEnabled = true;
                UpdateTearingEffect();
            }
            else if(command == FrameRateControl)
            {
                framePeriodMicroseconds = 1000000 / FrameRate((byte)(data & 0x1F));
                if(tearingEffectEnabled && !sleeping)
                {
                    UpdateTearingEffect();
                }
            }
            else if(command == MemoryWrite)
            {
                WritePixel(data);
            }
        }

        public IReadOnlyDictionary<int, IGPIO> Connections { get; }

        public long Size => DataOffset + 2;

        private void ResetController()
        {
            command = 0;
            parameters = 0;
            xStart = 0;
            xEnd = Width - 1;
            yStart = 0;
            yEnd = Height - 1;
            x = 0;
            y = 0;
            sleeping = false;
            tearingEffectEnabled = false;
            madctl = 0;
            pulseActive = false;
            framePeriodMicroseconds = 1000000 / DefaultFrameRate;
            tearGeneration++;
            Connections[0].Unset();
        }

        private void UpdateTearingEffect()
        {
            var enabled = tearingEffectEnabled && !sleeping;
            tearGeneration++;
            if(enabled)
            {
                ScheduleTearFrame(tearGeneration);
            }
            else
            {
                EndTearPulse();
            }
        }

        private void ScheduleTearFrame(uint generation)
        {
            machine.ScheduleAction(TimeInterval.FromMicroseconds(framePeriodMicroseconds), _ =>
            {
                if(generation != tearGeneration || !tearingEffectEnabled || sleeping)
                {
                    return;
                }
                if(framebufferDirty)
                {
                    PublishFrame();
                }
                pulseActive = true;
                Connections[0].Set(true);
                ScheduleTearFrame(generation);
                machine.ScheduleAction(TimeInterval.FromMilliseconds(TearPulseMilliseconds), __ => EndTearPulse(), "LCD TE pulse");
            }, "LCD TE frame");
        }

        private void EndTearPulse()
        {
            if(pulseActive)
            {
                pulseActive = false;
                Connections[0].Unset();
            }
        }

        private void WritePixel(ushort value)
        {
            var displayX = x;
            var displayY = y;
            if((madctl & MemoryVerticalAddressing) == 0)
            {
                displayX = Width - 1 - y;
                displayY = x;
            }

            if(displayX >= 0 && displayX < Width && displayY >= 0 && displayY < Height)
            {
                var pixel = 3 * (displayY * Width + displayX);
                framebuffer[pixel] = (byte)(((value >> 11) & 0x1F) * 255 / 31);
                framebuffer[pixel + 1] = (byte)(((value >> 5) & 0x3F) * 255 / 63);
                framebuffer[pixel + 2] = (byte)((value & 0x1F) * 255 / 31);
                framebufferDirty = true;
            }
            AdvanceCursor();
        }

        private void AdvanceCursor()
        {
            x++;
            if(x <= xEnd)
            {
                return;
            }

            x = xStart;
            y++;
            if(y <= yEnd)
            {
                return;
            }

            if(!tearingEffectEnabled)
            {
                PublishFrame();
            }
            y = yStart;
        }

        private void PublishFrame()
        {
            using(var image = new FileStream(ppmTemporaryPath, FileMode.Create, FileAccess.Write, FileShare.None))
            {
                var header = Encoding.ASCII.GetBytes($"P6\n{Width} {Height}\n255\n");
                image.Write(header, 0, header.Length);
                image.Write(framebuffer, 0, framebuffer.Length);
            }
            if(File.Exists(ppmPath))
            {
                File.Delete(ppmPath);
            }
            File.Move(ppmTemporaryPath, ppmPath);
            framebufferDirty = false;
        }

        private static ulong FrameRate(byte code)
        {
            return frameRates[code];
        }

        private readonly Machine machine;
        private readonly StreamWriter busLog;
        private readonly string ppmPath;
        private readonly string ppmTemporaryPath;
        private readonly byte[] framebuffer = new byte[Width * Height * 3];
        private readonly byte[] windowParameters = new byte[4];
        private readonly ushort[] displayId = { 0, 0x46, 0x01, 0x4e };
        private byte command;
        private int parameters;
        private int readIndex;
        private int xStart;
        private int xEnd;
        private int yStart;
        private int yEnd;
        private int x;
        private int y;
        private bool sleeping;
        private bool tearingEffectEnabled;
        private bool framebufferDirty;
        private byte madctl;
        private bool pulseActive;
        private uint tearGeneration;
        private ulong framePeriodMicroseconds;

        // The stock firmware selects MADCTL=0xA0, exposing the panel as 320x240.
        private const int Width = 320;
        private const int Height = 240;
        private const long CommandOffset = 0;
        private const long DataOffset = 0x20000;
        private const byte SoftwareReset = 0x01;
        private const byte ReadDisplayId = 0x04;
        private const byte SleepIn = 0x10;
        private const byte SleepOut = 0x11;
        private const byte MemoryWrite = 0x2C;
        private const byte MemoryRead = 0x2E;
        private const byte MemoryAccessControl = 0x36;
        private const byte MemoryVerticalAddressing = 0x20;
        private const byte TearingEffectOff = 0x34;
        private const byte TearingEffectOn = 0x35;
        private const byte ColumnAddressSet = 0x2A;
        private const byte RowAddressSet = 0x2B;
        private const byte FrameRateControl = 0xC6;
        private const ulong TearPulseMilliseconds = 1;
        private const ulong DefaultFrameRate = 60;

        private static readonly ulong[] frameRates =
        {
            119, 111, 105, 99, 94, 90, 86, 82, 78, 75, 72, 69, 67, 64, 62, 60,
            58, 57, 55, 53, 52, 50, 49, 48, 46, 45, 44, 43, 42, 41, 40, 39,
        };
    }
}
