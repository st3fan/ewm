
--
-- Intercept JSR COUT calls and turn A-Z into inverse. The key code is
-- in the accumulator so we can simply see if it is in the range we
-- are interested in and then shift it to the inverse character range.
--

JSR = 0x20
COUT = 0xfded

cpu.onBeforeExecuteInstruction(JSR, function(state, opcode, operand)
  if operand == COUT then
     if state.a >= 0xc1 and state.a <= 0xda then
        state.a = state.a - 0xC0
     end
  end
end)
