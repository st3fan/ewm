-- EWM Meets Lua

local function myerrhandler(err)
   print(err)
   print(debug.traceback())
   return false
end

xpcall(function()
   --
   -- Frogger.lua - Change the default Frogger key bindings.
   --
   -- Intercept LDA $C000. The key code will be in the
   -- accumulator. Map the values to our own preferences.
   --
   --   GETKEY: LDA $C000   ; AD 00 C0
   --           CMP #$80    ; C9 80
   --           BCC GETKEY  ; 90 F9
   --           STA $C010   ; 8D 10 C0
   --

   local cpu = require 'cpu'
   cpu.onAfterExecuteInstruction(0xAD, function(state, opcode, operand)
     if operand == 0xc000 then
       if state.a == 0xc9 then -- I
         state.a = 0xc1 -- A
       elseif state.a == 0xcb then -- K
         state.a = 0xda -- Z
       elseif state.a == 0xca then -- J
         state.a = 0x88 -- <-
       elseif state.a == 0xcc then -- L
         state.a = 0x95 -- ->
       end
     end
   end)

end , myerrhandler )
