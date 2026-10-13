local m=rawget(_G,"__mouse_studio_macro_v1") or {}
_G.__mouse_studio_macro_v1=m
function m.cancel()
  m.steps=nil; m.index=1; m.wait=0
  if m.timer then m.timer:set_enabled(false) end
end
function m.tick()
  if not m.steps then return end
  if m.wait>0 then m.wait=math.max(0,m.wait-20); if m.wait>0 then return end end
  -- At most 64 steps; immediate taps always have their corresponding release.
  while m.steps and m.index<=#m.steps do
    local step=m.steps[m.index]; m.index=m.index+1
    if step.delay then
      m.wait=step.delay
      if m.wait>0 then return end
    else
      hl.dispatch(hl.dsp.send_key_state({mods=step.mods,key=step.key,state="down"}))
      hl.dispatch(hl.dsp.send_key_state({mods=step.mods,key=step.key,state="up"}))
    end
  end
  m.cancel()
end
if not m.timer or m.timer:is_enabled()==nil then
  m.timer=hl.timer(function() m.tick() end,{timeout=20,type="repeat"})
  m.timer:set_enabled(false)
end
function m.start(steps)
  m.cancel(); m.steps=steps; m.index=1; m.timer:set_enabled(true)
end
