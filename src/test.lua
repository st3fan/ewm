-- EWM Meets Lua

cpu:onAfterExecuteInstruction(0xAD, function(cpu, opcode, operand)
   if operand == 0xc000 and cpu.a == 0xd2 then
      print(string.format("This is a %s", cpu.model))
      cpu:reset()
   end
end)

-- cpu.onBeforeExecuteInstruction(0x60, function(state, opcode, operand)
--   print(string.format('Before RTS from 0x%.4x', state.pc))
-- end)

-- cpu.onBeforeExecuteInstruction(0x20, function(state, opcode, operand)
--   print(string.format('Before JSR from 0x%.4x', state.pc))
-- end)

-- cpu.onBeforeExecuteInstruction(0xAD, function(state, opcode, operand)
--   print(string.format('Before LDA from 0x%.4x', state.pc))
-- end)

-- cpu.onAfterExecuteInstruction(0x60, function(state, opcode, operand)
--   print(string.format('After RTS from 0x%.4x', state.pc))
-- end)

-- cpu.onAfterExecuteInstruction(0x20, function(state, opcode, operand)
--   print(string.format('After JSR from 0x%.4x', state.pc))
-- end)

-- cpu.onAfterExecuteInstruction(0xAD, function(state, opcode, operand)
--   print(string.format('After LDA from 0x%.4x', state.pc))
-- end)
