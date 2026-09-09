// SPDX-License-Identifier: GPL-3.0-only

using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using Antmicro.Renode.Core;
using Antmicro.Renode.Peripherals;

namespace Antmicro.Renode.Peripherals
{
    // The N0120 keypad is a passive 9x6 switch matrix. PA drives open-drain
    // rows; PC senses columns pulled high. A held key only pulls its column
    // low while its row is driven low--it does not create an interrupt or key
    // event on its own.
    [GPIO(NumberOfInputs = Rows, NumberOfOutputs = Columns + 1)]
    public sealed class N0120KeyMatrix : IPeripheral, IGPIOReceiver, INumberedGPIOOutput
    {
        public N0120KeyMatrix()
        {
            var columns = new Dictionary<int, IGPIO>();
            for(var column = 0; column <= Columns; column++)
            {
                columns[column] = new GPIO();
            }
            Connections = new ReadOnlyDictionary<int, IGPIO>(columns);
            Reset();
        }

        public void Reset()
        {
            Array.Clear(pressed, 0, pressed.Length);
            for(var column = 0; column < Columns; column++)
            {
                Connections[column].Set(true);
            }
            Connections[WakeupOutput].Set(false);
        }

        public void OnGPIO(int row, bool value)
        {
            if(row < 0 || row >= Rows)
            {
                throw new ArgumentOutOfRangeException(nameof(row));
            }
            rowLevels[row] = value;
            UpdateColumns();
        }

        public void Press(string key)
        {
            SetKey(key, true);
        }

        public void Release(string key)
        {
            SetKey(key, false);
        }

        public IReadOnlyDictionary<int, IGPIO> Connections { get; }

        private void SetKey(string key, bool value)
        {
            if(!keyPositions.TryGetValue(key, out var position))
            {
                throw new ArgumentException($"Unknown N0120 key: {key}", nameof(key));
            }
            pressed[position / Columns, position % Columns] = value;
            if(position == 8)
            {
                Connections[WakeupOutput].Set(value);
            }
            UpdateColumns();
        }

        private void UpdateColumns()
        {
            for(var column = 0; column < Columns; column++)
            {
                var high = true;
                for(var row = 0; row < Rows; row++)
                {
                    if(pressed[row, column] && !rowLevels[row])
                    {
                        high = false;
                        break;
                    }
                }
                Connections[column].Set(high);
            }
        }

        private readonly bool[,] pressed = new bool[Rows, Columns];
        private readonly bool[] rowLevels = new bool[Rows];

        private const int Rows = 9;
        private const int Columns = 6;
        private const int WakeupOutput = Columns;

        private static readonly IReadOnlyDictionary<string, int> keyPositions =
            new Dictionary<string, int>(StringComparer.Ordinal)
            {
                ["Left"] = 0, ["Up"] = 1, ["Down"] = 2, ["Right"] = 3, ["OK"] = 4, ["Back"] = 5,
                ["Home"] = 6, ["Power"] = 8,
                ["Shift"] = 12, ["Alpha"] = 13, ["XNT"] = 14, ["Var"] = 15, ["Toolbox"] = 16, ["Backspace"] = 17,
                ["Exp"] = 18, ["Ln"] = 19, ["Log"] = 20, ["Imaginary"] = 21, ["Comma"] = 22, ["PowerKey"] = 23,
                ["Sin"] = 24, ["Cos"] = 25, ["Tan"] = 26, ["Pi"] = 27, ["Sqrt"] = 28, ["Square"] = 29,
                ["7"] = 30, ["8"] = 31, ["9"] = 32, ["LeftParenthesis"] = 33, ["RightParenthesis"] = 34,
                ["4"] = 36, ["5"] = 37, ["6"] = 38, ["Multiply"] = 39, ["Divide"] = 40,
                ["1"] = 42, ["2"] = 43, ["3"] = 44, ["Plus"] = 45, ["Minus"] = 46,
                ["0"] = 48, ["Dot"] = 49, ["EE"] = 50, ["Ans"] = 51, ["Equals"] = 52,
            };
    }
}
