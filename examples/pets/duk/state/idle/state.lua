-- local on_key_down = function (key)
--     print"Key down:"
--     print(key)
-- end

-- local on_key_up = function (key)
--     print"Key up:"
--     print(key)
-- end

-- The pet has entered the state.
function Init()
    print"Duk init"

    -- register_event_handler("key_down", on_key_down);
    -- register_event_handler("key_up", on_key_up);
end

-- A tick in the state.
function Update()
    if math.random(0, 100) < 1 then
        set_current_anim("quacking")
    elseif math.random(0, 100) < 4 then
        set_current_anim("blink")
    end
end

