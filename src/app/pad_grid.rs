pub const BANK_COUNT: usize = 2;
pub const PADS_PER_BANK: usize = 64;

const BUTTONS_BY_IMAGE_INDEX: [usize; 128] = [
    28, 29, 30, 31, 60, 61, 62, 63, 24, 25, 26, 27, 56, 57, 58, 59, 20, 21, 22, 23, 52, 53, 54, 55,
    16, 17, 18, 19, 48, 49, 50, 51, 12, 13, 14, 15, 44, 45, 46, 47, 8, 9, 10, 11, 40, 41, 42, 43,
    4, 5, 6, 7, 36, 37, 38, 39, 0, 1, 2, 3, 32, 33, 34, 35, 92, 93, 94, 95, 124, 125, 126, 127, 88,
    89, 90, 91, 120, 121, 122, 123, 84, 85, 86, 87, 116, 117, 118, 119, 80, 81, 82, 83, 112, 113,
    114, 115, 76, 77, 78, 79, 108, 109, 110, 111, 72, 73, 74, 75, 104, 105, 106, 107, 68, 69, 70,
    71, 100, 101, 102, 103, 64, 65, 66, 67, 96, 97, 98, 99,
];

pub fn image_index_for_button(button_index: usize) -> usize {
    BUTTONS_BY_IMAGE_INDEX
        .iter()
        .position(|candidate| *candidate == button_index)
        .unwrap_or(0)
}

pub fn bank_slot_for_button(button_index: usize) -> usize {
    image_index_for_button(button_index) % PADS_PER_BANK
}

pub fn button_for_bank_slot(bank: usize, slot: usize) -> usize {
    let bank = bank % BANK_COUNT;
    let slot = slot.min(PADS_PER_BANK - 1);
    BUTTONS_BY_IMAGE_INDEX[bank * PADS_PER_BANK + slot]
}

pub fn button_for_image_index(image_index: usize) -> usize {
    BUTTONS_BY_IMAGE_INDEX[image_index.min(BUTTONS_BY_IMAGE_INDEX.len() - 1)]
}

pub fn move_button_within_bank(button_index: usize, bank: usize, dx: isize, dy: isize) -> usize {
    let bank = bank % BANK_COUNT;
    let bank_offset = bank * PADS_PER_BANK;
    let bank_image_index = image_index_for_button(button_index)
        .saturating_sub(bank_offset)
        .min(PADS_PER_BANK - 1);
    let row = bank_image_index / 8;
    let col = bank_image_index % 8;
    let next_row = (row as isize + dy).clamp(0, 7) as usize;
    let next_col = (col as isize + dx).clamp(0, 7) as usize;

    button_for_image_index(bank_offset + next_row * 8 + next_col)
}
