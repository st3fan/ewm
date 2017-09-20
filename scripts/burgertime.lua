-- The MIT License (MIT)
--
-- Copyright (c) 2015 Stefan Arentz - http:--github.com/st3fan/ewm
--
-- Permission is hereby granted, free of charge, to any person obtaining a copy
-- of this software and associated documentation files (the "Software"), to deal
-- in the Software without restriction, including without limitation the rights
-- to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
-- copies of the Software, and to permit persons to whom the Software is
-- furnished to do so, subject to the following conditions:
--
-- The above copyright notice and this permission notice shall be included in all
-- copies or substantial portions of the Software.
--
-- THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
-- IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
-- FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
-- AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
-- LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
-- OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
-- SOFTWARE.

function dumpZeroPage()
   for i = 0,15 do
      local a = (i * 16)
      local s = string.format( "%.4x: %.2x %.2x %.2x %.2x %.2x %.2x %.2x %.2x %.2x %.2x %.2x %.2x %.2x %.2x %.2x %.2x", (i * 16), cpu.memory[a], cpu.memory[a+1], cpu.memory[a+2], cpu.memory[a+3], cpu.memory[a+4], cpu.memory[a+5], cpu.memory[a+6], cpu.memory[a+7], cpu.memory[a+8], cpu.memory[a+9], cpu.memory[a+10], cpu.memory[a+11], cpu.memory[a+12], cpu.memory[a+13], cpu.memory[a+14], cpu.memory[a+15])
      print(s)
   end
end

KMOD_GUI = 1024
KOMD_CTRL = 64

CUR_LIVES = 0x66
MAX_LIVES = 0x67

CUR_SHOTS = 0x6a
MAX_SHOTS = 0x6b

two:onKeyDown(function(two, mod, sym)
   if mod == KMOD_GUI then
      if sym == string.byte('z') then
         dumpZeroPage()
         return true
      end
      if sym == string.byte('r') then
         cpu.memory[CUR_LIVES] = 7
         cpu.memory[MAX_LIVES] = 7
         cpu.memory[CUR_SHOTS] = 7
         cpu.memory[MAX_SHOTS] = 7
         return true;
      end
   end
   return false
end)

STA_zpg_X = 0x95

cpu:onBeforeExecuteInstruction(STA_zpg_X, function(cpu, opcode, operand)
   if operand == CUR_LIVES or operand == CUR_SHOTS then
      cpu.a = 5
   end
end)
