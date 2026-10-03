// SPDX-License-Identifier: GPL-3.0-only

using System;
using System.Collections.Generic;
using Antmicro.Renode.Core;

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
            Connections = columns;
            Reset();
        }

        public void Reset()
        {
            for(var column = 0; column < Columns; column++)
            {
                Connections[column].Set(true);
            }
            Connections[WakeupOutput].Set(pressed[8]);
        }

        public void OnGPIO(int row, bool value)
        {
            rowLevels[row] = value;
            UpdateColumns();
        }

        public void Press(string key) => SetKey(key, true);

        public void Release(string key) => SetKey(key, false);

        public IReadOnlyDictionary<int, IGPIO> Connections { get; }

        private void SetKey(string key, bool value)
        {
            var position = Array.IndexOf(keys, key);
            if(position < 0)
            {
                throw new ArgumentException($"Unknown N0120 key: {key}", nameof(key));
            }
            pressed[position] = value;
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
                    if(pressed[row * Columns + column] && !rowLevels[row])
                    {
                        high = false;
                        break;
                    }
                }
                Connections[column].Set(high);
            }
        }

        private readonly bool[] pressed = new bool[Rows * Columns];
        private readonly bool[] rowLevels = new bool[Rows];

        private const int Rows = 9;
        private const int Columns = 6;
        private const int WakeupOutput = Columns;

        private static readonly string[] keys =
        {
            "Left", "Up", "Down", "Right", "OK", "Back", "Home", null, "Power", null, null, null,
            "Shift", "Alpha", "XNT", "Var", "Toolbox", "Backspace",
            "Exp", "Ln", "Log", "Imaginary", "Comma", "PowerKey",
            "Sin", "Cos", "Tan", "Pi", "Sqrt", "Square",
            "7", "8", "9", "LeftParenthesis", "RightParenthesis", null,
            "4", "5", "6", "Multiply", "Divide", null,
            "1", "2", "3", "Plus", "Minus", null,
            "0", "Dot", "EE", "Ans", "Equals",
        };
    }
}
