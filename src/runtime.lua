local s=rawget(_G,"__mouse_studio_v1") or {}
local button_count=3
_G.__mouse_studio_v1=s
-- Disable only old owned handles. Never remove/unbind a key.
for _,h in pairs(s.binds or {}) do h:set_enabled(false) end
s.binds=s.binds or {}
local function apply_pointer(p, values)
  for _,name in ipairs(p.pointer_names or {p.pointer.name}) do
    local copy={}; for k,v in pairs(values) do copy[k]=v end
    copy.name=name; hl.device(copy)
  end
end
local function fire(action) if action then hl.dispatch(action) end end
local function wheel_enabled(enabled)
  for _,key in ipairs({"wheel_up","wheel_down"}) do
    if s.binds[key] then s.binds[key]:set_enabled(enabled) end
  end
end
function s.cancel()
  if s.profile and s.precise then apply_pointer(s.profile,s.profile.pointer) end
  s.precise=false; s.flash={0,0,0,0,0,0,0,0}; s.pressed={false,false,false,false,false,false,false,false}; s.buttons={}; s.pending={}
  wheel_enabled(false)
  __mouse_studio_macro_v1.cancel()
  if s.timer then s.timer:set_enabled(false) end
end
s.cancel()
local receiver_route=string.find(input_names[1],"logitech-usb-receiver",1,true)==1
if receiver_route then
  local owner=rawget(_G,"__mouse_studio_receiver_owner")
  if owner and owner~=s then
    owner.cancel()
    for _,h in pairs(owner.binds or {}) do h:set_enabled(false) end
    if owner.focus_event then owner.focus_event:remove() end
    owner.address=nil
  end
  _G.__mouse_studio_receiver_owner=s
elseif rawget(_G,"__mouse_studio_receiver_owner")==s then
  _G.__mouse_studio_receiver_owner=nil
end
-- Device filters are read-only on Hyprland handles. Reuse a separate handle set
-- per transport, disabling the previous set without unbinding unrelated keys.
s.binding_routes=s.binding_routes or {}
local route=table.concat(input_names,"|")
s.binds=s.binding_routes[route] or {}
s.binding_routes[route]=s.binds
for _,p in ipairs(profiles) do p.pointer_names=input_names end
s.profiles=profiles; s.fallback=fallback; s.auto=auto; s.setup_token=setup_token; s.scroll_helper=scroll_helper
function s.cycle_dpi()
  if s.scroll_helper and s.profile then
    hl.dispatch(hl.dsp.exec_cmd(s.scroll_helper.." --cycle-dpi "..s.profile.dpi_presets))
  end
end
function s.toggle_wheel()
  if s.scroll_helper then hl.dispatch(hl.dsp.exec_cmd(s.scroll_helper.." --toggle-wheel")) end
end
function s.open_ring()
  -- The overlay is a separate process; it loads the saved profile's ring itself.
  if s.scroll_helper and s.profile then
    hl.dispatch(hl.dsp.exec_cmd(s.scroll_helper.." --ring "..s.profile_id.." "..device_slug))
  end
end
function s.activate(p)
  -- A button already down whose press reached the application must have its release pass through
  -- even if the new profile consumes that button.
  local passthrough={}
  for i=1,8 do if s.pressed[i] and not (s.consume and s.consume[i]) then passthrough[i]=true end end
  s.cancel(); s.passthrough=passthrough; s.profile=p; s.profile_id=p.id; s.token=p.token; s.actions=p.click
  apply_pointer(p,p.pointer)
  s.consume={}
  for i=1,button_count do
    local consumed=p.click[i]~=false or p.hold[i]~=false or p.double[i]~=false
      or p.precision==i or p.horizontal==i or p.gesture==i
    s.consume[i]=consumed
    if s.binds["press"..i] then
      s.binds["press"..i]:set_enabled(consumed)
      s.binds["release"..i]:set_enabled(consumed and not passthrough[i])
    end
  end
end
function s.focus(w)
  local chosen=nil
  if s.auto and w then
    local class=string.lower(w.class or "")
    for _,p in ipairs(s.profiles) do
      for _,match in ipairs(p.classes) do if class==match then chosen=p; break end end
      if chosen then break end
    end
  end
  if not chosen then for _,p in ipairs(s.profiles) do if p.id==s.fallback then chosen=p; break end end end
  if chosen and (not s.profile or chosen.id~=s.profile_id or chosen.token~=s.token) then s.activate(chosen) end
end
function s.press(i)
  if i<1 or i>8 then return end
  if s.pressed[i] then return end
  s.pressed[i]=true; s.flash[i]=260; s.timer:set_enabled(true)
  if i>button_count or not s.consume[i] then return end
  local p=s.profile; local cursor=hl.get_cursor_pos()
  s.buttons[i]={age=0,x=cursor.x,y=cursor.y,held=false,scrolled=false}
  if p.precision==i then
    s.precise=true
    apply_pointer(p,{sensitivity=p.precision_speed,accel_profile="flat"})
  elseif p.horizontal==i then wheel_enabled(true)
  elseif p.hold[i]~=false or p.double[i]~=false or p.gesture==i then s.timer:set_enabled(true)
  else fire(p.click[i]); s.buttons[i].fired=true end
end
function s.release(i)
  if not s.pressed[i] then return end
  s.pressed[i]=false
  if i>button_count then return end
  local p=s.profile; local b=s.buttons[i]; s.buttons[i]=nil
  if not b then return end
  if p.precision==i then s.precise=false; apply_pointer(p,p.pointer); return end
  if p.horizontal==i then wheel_enabled(false); if b.scrolled then return end end
  if b.held or b.fired then return end
  if p.gesture==i then
    local cursor=hl.get_cursor_pos(); local dx=cursor.x-b.x; local dy=cursor.y-b.y
    if math.max(math.abs(dx),math.abs(dy))>=p.threshold then
      local d=math.abs(dx)>math.abs(dy) and (dx<0 and 3 or 4) or (dy<0 and 1 or 2)
      fire(p.gestures[d]); return
    end
  end
  if p.double[i]~=false then
    if s.pending[i] then s.pending[i]=nil; fire(p.double[i])
    else s.pending[i]={age=0,action=p.click[i]}; s.timer:set_enabled(true) end
  else fire(p.click[i]) end
end
function s.tick()
  local work=false; local p=s.profile
  for i=1,8 do s.flash[i]=math.max(0,s.flash[i]-20); if s.flash[i]>0 then work=true end end
  for i=1,button_count do
    local b=s.buttons[i]
    if b then
      work=true; b.age=b.age+20
      if not b.held and p.hold[i]~=false and p.precision~=i and p.gesture~=i and p.horizontal~=i and b.age>=p.hold_ms then
        b.held=true; fire(p.hold[i])
      end
    end
    local pending=s.pending[i]
    if pending then
      pending.age=pending.age+20
      if pending.age>=p.double_ms then s.pending[i]=nil; fire(pending.action) else work=true end
    end
  end
  if not work then s.timer:set_enabled(false) end
end
function s.wheel(amount)
  local i=s.profile.horizontal
  if i~=0 and s.pressed[i] and s.scroll_helper then
    s.buttons[i].scrolled=true
    hl.dispatch(hl.dsp.exec_cmd(s.scroll_helper.." --horizontal-scroll "..amount))
  end
end
local function bind(key,keys,callback,opts)
  if not s.binds[key] or s.binds[key]:is_enabled()==nil then
    opts.device={inclusive=true,list={"logitech-mx-anywhere-3s"}}
    opts.ignore_mods=true; opts.description="Mouse Studio: "..key
    s.binds[key]=hl.bind(keys,callback,opts)
  end
  return s.binds[key]
end
local codes={[1]="mouse:275",[2]="mouse:276",[3]="mouse:274",[7]="mouse:272",[8]="mouse:273"}
s.handles={}
for i,code in pairs(codes) do
  if i<=3 then
    s.handles[i]=bind("press"..i,code,function() s.press(i) end,{})
    bind("release"..i,code,function() s.release(i) end,{release=true})
  end
  bind("watch_press"..i,code,function() if not s.consume or not s.consume[i] then s.press(i) end end,{non_consuming=true}):set_enabled(true)
  bind("watch_release"..i,code,function()
    if s.passthrough and s.passthrough[i] then
      s.passthrough[i]=nil
      if s.binds["release"..i] then s.binds["release"..i]:set_enabled(s.consume and s.consume[i] or false) end
    elseif not s.consume or not s.consume[i] then s.release(i) end
  end,{non_consuming=true,release=true}):set_enabled(true)
end
bind("wheel_up","mouse_up",function() s.wheel(1) end,{})
bind("wheel_down","mouse_down",function() s.wheel(-1) end,{})
if not s.timer or s.timer:is_enabled()==nil then
  s.timer=hl.timer(function() s.tick() end,{timeout=20,type="repeat"})
end
s.timer:set_enabled(false)
if not s.focus_event or not s.focus_event:is_active() then
  s.focus_event=hl.on("window.active",function(w) s.focus(w) end)
end
-- Force updating current data even when the profile ID stays unchanged.
s.profile=nil
s.focus(hl.get_active_window())
