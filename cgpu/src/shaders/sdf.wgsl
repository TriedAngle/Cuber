struct PushConstants {
    dimensions: vec3<i32>,
    num_steps: u32,
    current_step: u32,
}

struct BrickHandle {
    raw: u32,
}

var<push_constant> pc: PushConstants;

@group(0) @binding(0)
var<storage, read_write> brick_handles: array<BrickHandle>;

const EPSILON: f32 = 0.00001;

const DATA_BIT: u32 = 0x80000000u;  // Bit 31
const LOD_BIT: u32  = 0x40000000u;  // Bit 30 
const DATA_MASK: u32 = 0x7FFFFFFFu;  // Bits 0-30 for data
const EMPTY_DATA_MASK: u32 = 0x1FFFFFFFu; // Bits 0-29 for empty handle values
const MAX_DISTANCE: u32 = EMPTY_DATA_MASK;

fn brick_handle_is_empty(brick_handle: BrickHandle) -> bool {
    return (brick_handle.raw & DATA_BIT) == 0;
}

fn brick_handle_is_data(brick_handle: BrickHandle) -> bool {
    return (brick_handle.raw & DATA_BIT) == 1;
}

fn brick_handle_is_lod(brick_handle: BrickHandle) -> bool {
    return brick_handle_is_empty(brick_handle) && (brick_handle.raw & LOD_BIT) != 0;
}

fn is_empty(brick_handle: BrickHandle) -> bool {
    return brick_handle_is_empty(brick_handle) && !brick_handle_is_lod(brick_handle);
}

fn is_solid(brick_handle: BrickHandle) -> bool {
    return !brick_handle_is_empty(brick_handle) || brick_handle_is_lod(brick_handle);
}

fn brick_handle_get_empty_value(brick_handle: BrickHandle) -> u32 {
    return brick_handle.raw & EMPTY_DATA_MASK;
}

fn brick_handle_with_empty_value(value: u32) -> BrickHandle {
    return BrickHandle(value & EMPTY_DATA_MASK);
}


fn brick_handle_index(pos: vec3<i32>) -> u32 {
    return u32(
        pos.x + (pos.y * pc.dimensions.x) + (pos.z * pc.dimensions.x * pc.dimensions.y)
    );
}

fn get_brick_handle(pos: vec3<i32>) -> BrickHandle {
    let idx = brick_handle_index(pos);
    let brick_handle: BrickHandle = brick_handles[idx];
    return brick_handle;
}

fn set_brick_handle(pos: vec3<i32>, brick_handle: BrickHandle) {
    let idx = brick_handle_index(pos);
    brick_handles[idx] = brick_handle;
}


@compute @workgroup_size(8, 8, 4)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let pos = vec3<i32>(global_id);
    if any(pos >= pc.dimensions) {
        return;
    }

    let brick_handle = get_brick_handle(pos);

    if is_solid(brick_handle) {
        return;
    }

    if pc.current_step == 0u {
        set_brick_handle(pos, brick_handle_with_empty_value(MAX_DISTANCE));
        return;
    }


    var min_distance = f32(MAX_DISTANCE);

    let search_radius = max(
        max(pc.dimensions.x, pc.dimensions.y),
        pc.dimensions.z
    ) >> (pc.current_step - 1);

    if search_radius == 0 {
        return;
    }

    for (var z = -1i; z <= 1; z++) {
        for (var y = -1i; y <= 1; y++) {
            for (var x = -1i; x <= 1; x++) {
                let offset = vec3<i32>(x, y, z);
                let neighbor_pos = pos + offset * search_radius;

                if any(neighbor_pos < vec3<i32>(0)) || any(neighbor_pos >= pc.dimensions) {
                    continue;
                }

                let neighbor = get_brick_handle(neighbor_pos);

                if is_solid(neighbor) {
                    let distance = length(vec3<f32>(offset) * f32(search_radius));
                    min_distance = min(min_distance, distance);
                } else if is_empty(neighbor) {
                    let stored_distance = f32(brick_handle_get_empty_value(neighbor));
                    if stored_distance < f32(MAX_DISTANCE) {
                        let distance = length(vec3<f32>(offset) * f32(search_radius)) + stored_distance;
                        min_distance = min(min_distance, distance);
                    }
                }
            }
        }
    }

    if min_distance < f32(MAX_DISTANCE) {
        set_brick_handle(pos, brick_handle_with_empty_value(u32(min_distance)));
    }
}
