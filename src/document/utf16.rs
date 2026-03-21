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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_utf16_to_char_index_ascii() {
        let line = "hello world";
        assert_eq!(utf16_to_char_index(line, 0), 0);
        assert_eq!(utf16_to_char_index(line, 5), 5);
        assert_eq!(utf16_to_char_index(line, 11), 11);
    }

    #[test]
    fn test_utf16_to_char_index_emoji() {
        let line = "# 𝄞 test";
        // '# ' = 2 UTF-16, '𝄞' = 2 UTF-16, ' test' = 5 UTF-16
        assert_eq!(utf16_to_char_index(line, 0), 0); // '#'
        assert_eq!(utf16_to_char_index(line, 2), 2); // ' ' before emoji
        assert_eq!(utf16_to_char_index(line, 4), 3); // ' ' after emoji
        assert_eq!(utf16_to_char_index(line, 5), 4); // 't'
    }

    #[test]
    fn test_char_index_to_utf16_ascii() {
        let line = "hello world";
        assert_eq!(char_index_to_utf16(line, 0), 0);
        assert_eq!(char_index_to_utf16(line, 5), 5);
        assert_eq!(char_index_to_utf16(line, 11), 11);
    }

    #[test]
    fn test_char_index_to_utf16_emoji() {
        let line = "# 𝄞 test";
        assert_eq!(char_index_to_utf16(line, 0), 0); // at '#'
        assert_eq!(char_index_to_utf16(line, 3), 4); // after emoji (2 chars '#' + ' ' + emoji = 4 UTF-16)
        assert_eq!(char_index_to_utf16(line, 4), 5); // at space after emoji
    }
}
