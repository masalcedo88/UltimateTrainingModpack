use crate::common::consts::*;
use crate::common::*;
use crate::training::character_specific;
use crate::training::fast_fall;
use crate::training::frame_counter;
use crate::training::full_hop;
use crate::training::shield;
use smash::app::{self, lua_bind::*};
use smash::lib::lua_const::*;

static mut CURRENT_AERIAL: Action = Action::NAIR;
static mut QUEUE: Vec<Action> = vec![];

static mut FALLING_AERIAL: bool = false;

static mut AERIAL_DELAY_COUNTER: usize = 0;
static mut AERIAL_DELAY: u32 = 0;

pub fn buffer_action(action: Action) {
    unsafe {
        if QUEUE.len() > 0 {
            return;
        }
    }

    if action == Action::empty() {
        return;
    }

    roll_aerial_delay(action);

    unsafe {
        QUEUE.insert(0, action);
        buffer_follow_up();
    }
}

pub fn buffer_follow_up() {
    let action;

    unsafe {
        action = MENU.follow_up.get_random();
    }

    if action == Action::empty() {
        return;
    }

    roll_aerial_delay(action);

    unsafe {
        QUEUE.insert(0, action);
    }
}

pub fn get_current_buffer() -> Action {
    unsafe {
        if QUEUE.len() == 0 {
            return Action::empty();
        }

        return *QUEUE.last().unwrap();
    }
}

fn reset() {
    unsafe {
        QUEUE.pop();
    }

    shield::suspend_shield(get_current_buffer());

    unsafe {
        frame_counter::full_reset(AERIAL_DELAY_COUNTER);
        AERIAL_DELAY = 0;
    }
}

pub fn full_reset() {
    unsafe {
        while QUEUE.len() > 0 {
            reset();
        }
    }
}

pub fn set_aerial(attack: Action) {
    unsafe {
        CURRENT_AERIAL = attack;
    }
}

pub unsafe fn get_attack_air_kind(
    module_accessor: &mut app::BattleObjectModuleAccessor,
) -> Option<i32> {
    if !is_operation_cpu(module_accessor) {
        return None;
    }

    CURRENT_AERIAL.into_attack_air_kind()
}

pub unsafe fn get_command_flag_cat(
    module_accessor: &mut app::BattleObjectModuleAccessor,
    category: i32,
) -> i32 {
    // Only do once per frame
    if category != FIGHTER_PAD_COMMAND_CATEGORY1 {
        return 0;
    }

    if !is_operation_cpu(module_accessor) {
        return 0;
    }

    check_buffer(module_accessor);

    return perform_action(module_accessor);
}

unsafe fn check_buffer(module_accessor: &mut app::BattleObjectModuleAccessor) {
    if QUEUE.len() > 0 {
        return;
    }

    if !should_buffer(module_accessor) {
        return;
    }

    buffer_menu_mash();
}

fn should_buffer(module_accessor: &mut app::BattleObjectModuleAccessor) -> bool {
    unsafe {
        if MENU.mash_in_neutral == OnOff::On {
            return true;
        }
    }

    if is_in_hitstun(module_accessor) {
        return true;
    }

    if is_in_footstool(module_accessor) {
        return true;
    }

    return false;
}

// Temp Translation
pub fn buffer_menu_mash() -> Action {
    unsafe {
        let action = MENU.mash_state.get_random();
        buffer_action(action);

        full_hop::roll_full_hop();
        fast_fall::roll_fast_fall();
        FALLING_AERIAL = MENU.falling_aerials.get_random().into_bool();

        action
    }
}

unsafe fn perform_action(module_accessor: &mut app::BattleObjectModuleAccessor) -> i32 {
    let action = get_current_buffer();

    match action {
        Action::AIR_DODGE => {
            let expected_status;
            let command_flag;
            // Shield if grounded instead
            if is_grounded(module_accessor) {
                /*
                Doesn't actually cause the shield, but will clear the buffer once shield is possible.
                Shield hold is performed through shield::should_hold_shield and request_shield
                */
                expected_status = *FIGHTER_STATUS_KIND_GUARD_ON;
                command_flag = *FIGHTER_PAD_CMD_CAT1_FLAG_AIR_ESCAPE;
            } else {
                expected_status = *FIGHTER_STATUS_KIND_ESCAPE_AIR;
                command_flag = *FIGHTER_PAD_CMD_CAT1_FLAG_AIR_ESCAPE;
            }

            return get_flag(module_accessor, expected_status, command_flag);
        }
        Action::JUMP => {
            return update_jump_flag(module_accessor);
        }
        Action::SPOT_DODGE => {
            return get_flag(
                module_accessor,
                *FIGHTER_STATUS_KIND_ESCAPE,
                *FIGHTER_PAD_CMD_CAT1_FLAG_ESCAPE,
            );
        }
        Action::ROLL_F => {
            return get_flag(
                module_accessor,
                *FIGHTER_STATUS_KIND_ESCAPE_F,
                *FIGHTER_PAD_CMD_CAT1_FLAG_ESCAPE_F,
            );
        }
        Action::ROLL_B => {
            return get_flag(
                module_accessor,
                *FIGHTER_STATUS_KIND_ESCAPE_B,
                *FIGHTER_PAD_CMD_CAT1_FLAG_ESCAPE_B,
            );
        }
        Action::SHIELD => {
            if !is_grounded(module_accessor) {
                return 0;
            }
            /*
            Doesn't actually cause the shield, but will clear the buffer once shield is possible.
            Shield hold is performed through shield::should_hold_shield and request_shield
            */
            return get_flag(
                module_accessor,
                *FIGHTER_STATUS_KIND_GUARD_ON,
                *FIGHTER_PAD_CMD_CAT1_FLAG_AIR_ESCAPE,
            );
        }
        Action::DASH => {
            let dash_transition = *FIGHTER_STATUS_TRANSITION_TERM_ID_CONT_DASH;
            let dash_status = *FIGHTER_STATUS_KIND_DASH;

            try_change_status(module_accessor, dash_status, dash_transition);

            return get_flag(module_accessor, *FIGHTER_STATUS_KIND_DASH, 0);
        }
        _ => return get_attack_flag(module_accessor, action),
    }
}

pub fn request_shield(module_accessor: &mut app::BattleObjectModuleAccessor) -> bool {
    match get_current_buffer() {
        Action::SHIELD => return true,
        Action::AIR_DODGE => return is_grounded(module_accessor),
        _ => {}
    }

    return false;
}

unsafe fn update_jump_flag(module_accessor: &mut app::BattleObjectModuleAccessor) -> i32 {
    let check_flag = if is_grounded(module_accessor) {
        *FIGHTER_STATUS_KIND_JUMP_SQUAT
    } else if is_airborne(module_accessor) {
        *FIGHTER_STATUS_KIND_JUMP_AERIAL
    } else {
        *FIGHTER_STATUS_KIND_JUMP
    };
    let command_flag = *FIGHTER_PAD_CMD_CAT1_FLAG_JUMP_BUTTON;

    return get_flag(module_accessor, check_flag, command_flag);
}

unsafe fn get_attack_flag(
    module_accessor: &mut app::BattleObjectModuleAccessor,
    action: Action,
) -> i32 {
    let command_flag: i32;
    let status: i32;

    match action {
        Action::NAIR | Action::FAIR | Action::BAIR | Action::UAIR | Action::DAIR => {
            return get_aerial_flag(module_accessor, action);
        }
        Action::NEUTRAL_B => {
            command_flag = *FIGHTER_PAD_CMD_CAT1_FLAG_SPECIAL_N;
            status = *FIGHTER_STATUS_KIND_SPECIAL_N;
        }
        Action::SIDE_B => {
            command_flag = *FIGHTER_PAD_CMD_CAT1_FLAG_SPECIAL_S;
            status = *FIGHTER_STATUS_KIND_SPECIAL_S;
        }
        Action::UP_B => {
            command_flag = *FIGHTER_PAD_CMD_CAT1_FLAG_SPECIAL_HI;
            status = *FIGHTER_STATUS_KIND_SPECIAL_HI;
        }
        Action::DOWN_B => {
            command_flag = *FIGHTER_PAD_CMD_CAT1_FLAG_SPECIAL_LW;
            status = *FIGHTER_STATUS_KIND_SPECIAL_LW;
        }
        Action::U_SMASH => {
            command_flag = *FIGHTER_PAD_CMD_CAT1_FLAG_ATTACK_HI4;
            status = *FIGHTER_STATUS_KIND_ATTACK_HI4_START;
        }
        Action::F_SMASH => {
            command_flag = *FIGHTER_PAD_CMD_CAT1_FLAG_ATTACK_S4;
            status = *FIGHTER_STATUS_KIND_ATTACK_S4_START;
        }
        Action::D_SMASH => {
            command_flag = *FIGHTER_PAD_CMD_CAT1_FLAG_ATTACK_LW4;
            status = *FIGHTER_STATUS_KIND_ATTACK_LW4_START;
        }
        Action::GRAB => {
            let cannot_grab = WorkModule::get_int(
                module_accessor,
                *FIGHTER_INSTANCE_WORK_ID_INT_INVALID_CATCH_FRAME,
            ) != 0;
            if cannot_grab {
                return 0;
            }

            command_flag = *FIGHTER_PAD_CMD_CAT1_FLAG_CATCH;
            status = *FIGHTER_STATUS_KIND_CATCH;
        }
        Action::JAB => {
            // Prevent nair when airborne
            if !is_grounded(module_accessor) {
                return 0;
            }

            command_flag = *FIGHTER_PAD_CMD_CAT1_FLAG_ATTACK_N;
            status = *FIGHTER_STATUS_KIND_ATTACK;
        }
        Action::F_TILT => {
            command_flag = *FIGHTER_PAD_CMD_CAT1_FLAG_ATTACK_S3;
            status = *FIGHTER_STATUS_KIND_ATTACK_S3;
        }
        Action::U_TILT => {
            command_flag = *FIGHTER_PAD_CMD_CAT1_FLAG_ATTACK_HI3;
            status = *FIGHTER_STATUS_KIND_ATTACK_HI3;
        }
        Action::D_TILT => {
            command_flag = *FIGHTER_PAD_CMD_CAT1_FLAG_ATTACK_LW3;
            status = *FIGHTER_STATUS_KIND_ATTACK_LW3;
        }
        // TODO: Make it work, without being 1 frame late
        Action::DASH_ATTACK => {
            let current_status = StatusModule::status_kind(module_accessor);
            let dash_status = *FIGHTER_STATUS_KIND_DASH;
            let is_dashing = current_status == dash_status;

            // Start Dash First
            if !is_dashing {
                let dash_transition = *FIGHTER_STATUS_TRANSITION_TERM_ID_CONT_DASH;

                try_change_status(module_accessor, dash_status, dash_transition);
                return 0;
            }

            status = *FIGHTER_STATUS_KIND_ATTACK_DASH;

            let transition = *FIGHTER_STATUS_TRANSITION_TERM_ID_CONT_ATTACK_DASH;
            try_change_status(module_accessor, status, transition);

            //@TODO find out how to properly reset, since the status just returns FIGHTER_STATUS_KIND_DASH

            return 0;
        }
        _ => return 0,
    }

    return get_flag(module_accessor, status, command_flag);
}

unsafe fn get_aerial_flag(
    module_accessor: &mut app::BattleObjectModuleAccessor,
    action: Action,
) -> i32 {
    let mut flag: i32 = 0;

    // If we are grounded we also need to jump
    if is_grounded(module_accessor) {
        // let jump_flag = *FIGHTER_STATUS_KIND_JUMP_SQUAT;
        // let jump_transition = *FIGHTER_STATUS_TRANSITION_TERM_ID_CONT_JUMP_SQUAT;
        // try_change_status(module_accessor, jump_flag, jump_transition);
        flag |= *FIGHTER_PAD_CMD_CAT1_FLAG_JUMP_BUTTON;

        // Delay attack until we are airborne to get a full hop
        if full_hop::should_full_hop() {
            return flag;
        }
    }

    let status = *FIGHTER_STATUS_KIND_ATTACK_AIR;

    if FALLING_AERIAL && !fast_fall::is_falling(module_accessor) {
        return flag;
    }

    if should_delay_aerial(module_accessor) {
        return flag;
    }

    /*
     * We always trigger attack and change it later into the correct aerial
     * @see get_attack_air_kind()
     */
    let command_flag: i32 = match action {
        Action::NAIR | Action::FAIR | Action::BAIR | Action::UAIR | Action::DAIR => {
            *FIGHTER_PAD_CMD_CAT1_FLAG_ATTACK_N
        }
        _ => 0,
    };

    set_aerial(action);

    flag |= get_flag(module_accessor, status, command_flag);

    return flag;
}

pub fn init() {
    unsafe {
        AERIAL_DELAY_COUNTER = frame_counter::register_counter();
    }
}

fn roll_aerial_delay(action: Action) {
    if !shield::is_aerial(action) {
        return;
    }
    unsafe {
        AERIAL_DELAY = MENU.aerial_delay.get_random().to_index();
    }
}

fn should_delay_aerial(module_accessor: &mut app::BattleObjectModuleAccessor) -> bool {
    unsafe {
        if AERIAL_DELAY == 0 {
            return false;
        }

        if StatusModule::status_kind(module_accessor) == *FIGHTER_STATUS_KIND_ATTACK_AIR {
            return false;
        }

        if !WorkModule::is_enable_transition_term(
            module_accessor,
            *FIGHTER_STATUS_TRANSITION_TERM_ID_CONT_ATTACK_AIR,
        ) {
            return true;
        }

        return frame_counter::should_delay(AERIAL_DELAY, AERIAL_DELAY_COUNTER);
    }
}

/**
 * Returns the flag and resets, once the action is performed
 */
unsafe fn get_flag(
    module_accessor: &mut app::BattleObjectModuleAccessor,
    expected_status: i32,
    command_flag: i32,
) -> i32 {
    // let current_status = StatusModule::prev_status_kind(module_accessor,0);
    let current_status = StatusModule::status_kind(module_accessor);
    if current_status == expected_status {
        // Reset Buffer
        reset();
    }

    // Workaround for Character specific status
    if character_specific::check_status(module_accessor, current_status, expected_status) {
        reset();
    }

    return command_flag;
}

fn try_change_status(
    module_accessor: &mut app::BattleObjectModuleAccessor,
    expected_status: i32,
    transition_flag: i32,
) -> bool {
    let allow_transition;
    unsafe {
        allow_transition = WorkModule::is_enable_transition_term(module_accessor, transition_flag);
    }

    if !allow_transition {
        return false;
    }

    unsafe {
        StatusModule::change_status_request_from_script(module_accessor, expected_status, true);
    }

    return true;
}

pub unsafe fn perform_defensive_option() {
    full_reset();

    let action = match MENU.defensive_state.get_random() {
        Defensive::ROLL_F => Action::ROLL_F,
        Defensive::ROLL_B => Action::ROLL_B,
        Defensive::SPOT_DODGE => Action::SPOT_DODGE,
        Defensive::JAB => Action::JAB,
        Defensive::SHIELD => Action::SHIELD,
        _ => Action::empty(),
    };

    buffer_action(action);

    // Suspend shield hold to allow for other defensive options
    shield::suspend_shield(action);
}
