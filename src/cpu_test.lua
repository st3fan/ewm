-- EWM Meets Lua

local function myerrhandler(err)
   print(err)
   print(debug.traceback())
   return false
end

-- TODO How do we do this in C?
xpcall(function()
   -- Add some random intercepts to measure the performance impact

   local cpu = require 'cpu'

   cpu.onBeforeExecuteInstruction(0x60, function(state, opcode, operand)
     --print(string.format('Before RTS from 0x%.4x', state.pc))
   end)

   cpu.onBeforeExecuteInstruction(0x20, function(state, opcode, operand)
     --print(string.format('Before JSR from 0x%.4x', state.pc))
   end)

   cpu.onAfterExecuteInstruction(0x60, function(state, opcode, operand)
     --print(string.format('After RTS from 0x%.4x', state.pc))
   end)

   cpu.onAfterExecuteInstruction(0x20, function(state, opcode, operand)
     --print(string.format('After JSR from 0x%.4x', state.pc))
   end)

end , myerrhandler)
