// SPDX-License-Identifier: GPL-3.0-only

using System;
using System.Collections.Generic;
using System.IO;
using System.IO.MemoryMappedFiles;
using System.Threading;
using Antmicro.Renode.Core;
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
            Connections = new Dictionary<int, IGPIO> { [0] = new GPIO() };

            var sharedPath = Environment.GetEnvironmentVariable("LIBRECALC_LCD_SHM");
            if(!string.IsNullOrEmpty(sharedPath))
            {
                sharedMemory = MemoryMappedFile.CreateFromFile(sharedPath, FileMode.Open, null,
                    SharedMemorySize, MemoryMappedFileAccess.ReadWrite);
                sharedView = sharedMemory.CreateViewAccessor(0, SharedMemorySize, MemoryMappedFileAccess.ReadWrite);
                sharedView.Write(0, 0x444C434Cu);
                sharedView.Write(4, (uint)Width);
                sharedView.Write(8, (uint)Height);
            }

            Reset();
        }

        public void Reset()
        {
            ResetController();
            Array.Clear(framebuffer, 0, framebuffer.Length);
            framebufferDirty = true;
            PublishFrame();
        }

        public ushort ReadWord(long offset)
        {
            if(offset != DataOffset)
            {
                return 0;
            }
            if(command == 0x04) // Read display ID
            {
                return displayId[readIndex++ % displayId.Length];
            }
            if(command == MemoryRead)
            {
                if(readIndex++ == 0)
                {
                    return 0;
                }

                var (displayX, displayY) = DisplayPosition;
                var pixel = 3 * (displayY * Width + displayX);
                AdvanceCursor();
                return (ushort)((framebuffer[pixel] * 31 + 127) / 255 << 11
                    | (framebuffer[pixel + 1] * 63 + 127) / 255 << 5
                    | (framebuffer[pixel + 2] * 31 + 127) / 255);
            }
            return 0;
        }

        public void WriteWord(long offset, ushort value)
        {
            if(offset == 0)
            {
                command = (byte)value;
                parameters = 0;
                readIndex = 0;
                if(command == 0x01) // Software reset
                {
                    ResetController();
                }
                else if(command == 0x10) // Sleep in
                {
                    sleeping = true;
                    UpdateTearingEffect();
                }
                else if(command == 0x11) // Sleep out
                {
                    sleeping = false;
                    UpdateTearingEffect();
                }
                else if(command == 0x34) // Tearing effect off
                {
                    tearingEffectEnabled = false;
                    if(framebufferDirty)
                    {
                        PublishFrame();
                    }
                    UpdateTearingEffect();
                }
                else if(command == MemoryWrite || command == MemoryRead)
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

            if(command == ColumnAddressSet || command == 0x2B) // Row address set
            {
                windowParameters[parameters++] = (byte)value;
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
                    parameters = 0;
                }
            }
            else if(command == 0x36) // Memory access control
            {
                madctl = (byte)value;
            }
            else if(command == 0x35) // Tearing effect on
            {
                tearingEffectEnabled = true;
                UpdateTearingEffect();
            }
            else if(command == 0xC6) // Frame rate control
            {
                framePeriodMicroseconds = 1000000 / frameRates[value & 0x1F];
                UpdateTearingEffect();
            }
            else if(command == MemoryWrite)
            {
                var (displayX, displayY) = DisplayPosition;
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
        }

        public IReadOnlyDictionary<int, IGPIO> Connections { get; }

        public long Size => DataOffset + 2;

        private (int x, int y) DisplayPosition => (madctl & 0x20) == 0 ? (Width - 1 - y, x) : (x, y);

        private void ResetController()
        {
            command = 0;
            parameters = xStart = yStart = x = y = 0;
            xEnd = Width - 1;
            yEnd = Height - 1;
            sleeping = tearingEffectEnabled = false;
            madctl = 0;
            framePeriodMicroseconds = 1000000 / 60;
            Connections[0].Unset();
            UpdateTearingEffect();
        }

        private void UpdateTearingEffect()
        {
            if(!tearingEffectEnabled || sleeping)
            {
                Connections[0].Unset();
            }
            ScheduleFrame(++tearGeneration);
        }

        private void ScheduleFrame(uint generation)
        {
            machine.ScheduleAction(TimeInterval.FromMicroseconds(framePeriodMicroseconds), _ =>
            {
                if(generation != tearGeneration)
                {
                    return;
                }
                // Retry a coalesced frame even after the guest stops drawing.
                if(framebufferDirty)
                {
                    PublishFrame();
                }
                if(tearingEffectEnabled && !sleeping)
                {
                    Connections[0].Set(true);
                    machine.ScheduleAction(TimeInterval.FromMilliseconds(1), __ => Connections[0].Unset(), "LCD TE pulse");
                }
                ScheduleFrame(generation);
            }, "LCD frame");
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
            if(sharedView == null)
            {
                framebufferDirty = false;
                return;
            }
            if(sharedView.ReadUInt32(20) != frameSequence)
            {
                return;
            }
            Thread.MemoryBarrier();
            var nextBuffer = 1 - activeBuffer;
            sharedView.WriteArray(HeaderSize + nextBuffer * framebuffer.Length,
                framebuffer, 0, framebuffer.Length);
            Thread.MemoryBarrier();
            activeBuffer = nextBuffer;
            frameSequence++;
            sharedView.Write(12, (uint)activeBuffer);
            sharedView.Write(16, frameSequence);
            framebufferDirty = false;
        }

        private readonly Machine machine;
        private readonly MemoryMappedFile sharedMemory = null;
        private readonly MemoryMappedViewAccessor sharedView = null;
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
        private int activeBuffer;
        private uint tearGeneration;
        private uint frameSequence;
        private ulong framePeriodMicroseconds;

        // The stock firmware selects MADCTL=0xA0, exposing the panel as 320x240.
        private const int Width = 320;
        private const int Height = 240;
        private const int HeaderSize = 64;
        private const long SharedMemorySize = HeaderSize + 2 * Width * Height * 3;
        private const long DataOffset = 0x20000;
        private const byte MemoryWrite = 0x2C;
        private const byte MemoryRead = 0x2E;
        private const byte ColumnAddressSet = 0x2A;

        private static readonly ulong[] frameRates =
        {
            119, 111, 105, 99, 94, 90, 86, 82, 78, 75, 72, 69, 67, 64, 62, 60,
            58, 57, 55, 53, 52, 50, 49, 48, 46, 45, 44, 43, 42, 41, 40, 39,
        };
    }
}
