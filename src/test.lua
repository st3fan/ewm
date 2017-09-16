-- EWM Meets Lua

local function myerrhandler(err)
   print(err)
   print(debug.traceback())
   return false
end

xpcall(function()

   local two = require 'two'
   two.hello()

   local cpu = require 'cpu'
   cpu.hello()

   local dsk = require 'dsk'
   dsk.hello()

   -- Intercept JSR COUT calls
   -- cpu.onBeforeExecuteInstruction(0x20, function(cpu, opcode, operand)
   --   if operand == 0xfded then
   --      print(string.format("COUT was called from %.4x with A=%.2x", cpu:pc(), cpu:a()))
   --      if cpu:a() >= 0xc1 and cpu:a() <= 0xda  then
   --         -- Lets inverse this character
   --         cpu:a(cpu:a() - 0xC0)
   --      end
   --   end
   -- end)

   cpu.onBeforeExecuteInstruction(0x20, function(state, opcode, operand)
     if operand == 0xfded then
        print(string.format("COUT was called from %.4x with A=%.2x", state.a, state.pc))
        if state.a >= 0xc1 and state.a <= 0xda  then
           -- Lets inverse this character
           state.a = state.a - 0xC0
        end
     end
   end)

   -- Reset when the R key is pressed
   --two.onKeyDown('R', function(two, key)
   --  print('You pressed R')
   --  two.reset()
   --end)

   -- Enter the Monitor when the M key is pressed
   --two.onKeyDown('M', function(two, key)
   --  print('You pressed B')
   --  two.monitor()
   --end)

end , myerrhandler )
