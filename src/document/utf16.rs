pub fn utf16_to_char_index(line: &str, utf16_offset: u32) -> usize {
    let mut char_index = 0;
    let mut utf16_count = 0;

    for ch in line.chars() {
        if utf16_count >= utf16_offset {
            break;
        }
        utf16_count += ch.len_utf16() as u32;
        char_index += 1;
    }

    char_index.min(line.chars().count())
}

pub fn char_index_to_utf16(line: &str, char_index: usize) -> u32 {
    line.chars()
        .take(char_index)
        .map(|ch| ch.len_utf16() as u32)
        .sum()
}
