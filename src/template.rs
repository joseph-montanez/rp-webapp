use crate::http::ByteString;

const TEMPLATE_SIZE: usize = 1024;

const MAX_DICT_ENTRIES: usize = 10; // Adjust this based on your needs

#[derive(Copy, Clone, PartialEq)]
struct KeyValuePair<'a> {
    key: &'a str,
    value: &'a str,
}

#[derive(Copy, Clone)]
struct StaticDict<'a> {
    entries: [Option<KeyValuePair<'a>>; MAX_DICT_ENTRIES],
    count: usize,
}

impl<'a> StaticDict<'a> {
    pub fn new() -> Self {
        StaticDict {
            entries: [None; MAX_DICT_ENTRIES],
            count: 0,
        }
    }

    pub fn insert(&mut self, key: &'a str, value: &'a str) {
        if self.count < MAX_DICT_ENTRIES {
            self.entries[self.count] = Some(KeyValuePair { key, value });
            self.count += 1;
        }
        // Handle overflow case if necessary
    }

    pub fn get(&self, key: &str) -> Option<&'a str> {
        for entry in self.entries.iter() {
            if let Some(KeyValuePair { key: k, value: v }) = entry {
                if k == &key {
                    return Some(v);
                }
            }
        }
        None
    }
}


pub fn replace<const SIZE: usize>(template: &mut [u8; SIZE], placeholder: &str, value: &str) {
    let placeholder_bytes = placeholder.as_bytes();
    let value_bytes = value.as_bytes();
    let mut i = 0;

    while i < SIZE {
        if template[i..].starts_with(placeholder_bytes) {
            let diff = placeholder_bytes.len() as isize - value_bytes.len() as isize;

            // Replace placeholder with value
            for (j, &b) in value_bytes.iter().enumerate() {
                if i + j < SIZE {
                    template[i + j] = b;
                }
            }

            if diff > 0 {
                // Shift left if replacement is shorter
                let start = i + value_bytes.len();
                for j in start..SIZE - diff as usize {
                    template[j] = template[j + diff as usize];
                }
                template[SIZE - diff as usize..].fill(0);
            } else if diff < 0 {
                // Shift right if replacement is longer
                let shift_by = (-diff) as usize; // Make it a positive usize
                if i + placeholder_bytes.len() + shift_by < SIZE {
                    for j in (i + placeholder_bytes.len()..SIZE - shift_by).rev() {
                        template[j + shift_by] = template[j];
                    }
                } else {
                    // Not enough space to expand, break from loop
                    break;
                }
            }

            // Move index to the end of the replacement
            i += value_bytes.len();
            if diff < 0 {
                i += (-diff) as usize; // Adjust for longer replacements
            }
        } else {
            i += 1;
        }
    }
}

pub fn process_if<const SIZE: usize>(template: &mut [u8; SIZE], condition: bool, if_placeholder: &str, if_end_placeholder: &str, else_placeholder: Option<&str>, else_end_placeholder: Option<&str>) {
    if condition {
        // Replace if-placeholder with empty and remove if-end-placeholder
        replace(template, if_placeholder, "");
        replace(template, if_end_placeholder, "");

        // Remove entire else block if else placeholders are provided
        if let (Some(else_start), Some(else_end)) = (else_placeholder, else_end_placeholder) {
            remove_block(template, else_start, else_end);
        }
    } else {
        // Remove entire if block
        remove_block(template, if_placeholder, if_end_placeholder);

        // Process else block if else placeholders are provided
        if let (Some(else_start), Some(else_end)) = (else_placeholder, else_end_placeholder) {
            replace(template, else_start, "");
            replace(template, else_end, "");
        }
    }
}

fn remove_block<const SIZE: usize>(template: &mut [u8; SIZE], start_placeholder: &str, end_placeholder: &str) {
    let start_pos = find_position(template, start_placeholder);
    let end_pos = find_position(template, end_placeholder);

    if let (Some(start), Some(end)) = (start_pos, end_pos) {
        // Calculate length to remove
        let length_to_remove = end + end_placeholder.len() - start;

        // Shift the array to remove the block
        if start + length_to_remove < SIZE {
            template.copy_within(start + length_to_remove.., start);
        }

        // Zero out the shifted part
        let new_len = SIZE - length_to_remove;
        template[new_len..].fill(0);
    }
}

fn find_position<const SIZE: usize>(template: &[u8; SIZE], placeholder: &str) -> Option<usize> {
    let placeholder_bytes = placeholder.as_bytes();
    template.windows(placeholder_bytes.len()).position(|window| window == placeholder_bytes)
}

fn remove_marked_sections<const SIZE: usize>(template: &mut [u8; SIZE]) {
    // Find and remove "REMOVE" marked sections
    replace(template, "REMOVE", "");
}

pub fn process_for<const TEMPLATE_SIZE: usize>(
    template: &mut [u8; TEMPLATE_SIZE],
    loop_var: &str,
    item_list: &str,
    items: &[StaticDict])
{
    // Construct start tag
    let mut start_tag = ByteString::<128>::new(b"");
    start_tag.append(b"{{#for ");
    start_tag.append(loop_var.as_bytes());
    start_tag.append(b" in ");
    start_tag.append(item_list.as_bytes());
    start_tag.append(b"}}");
    let start_tag_str = core::str::from_utf8(&start_tag.as_bytes()).unwrap_or("");

    // Construct end tag
    let mut end_tag = ByteString::<128>::new(b"{{/for ");
    end_tag.append(loop_var.as_bytes());
    end_tag.append(b" in ");
    end_tag.append(item_list.as_bytes());
    end_tag.append(b"}}");
    let end_tag_str = core::str::from_utf8(&end_tag.as_bytes()).unwrap_or("");


    let start_pos = find_position(template, &start_tag_str);
    let end_pos = find_position(template, &end_tag_str);

    if let (Some(start), Some(end)) = (start_pos, end_pos) {
        let loop_content_length = end - start - start_tag.len();
        let mut loop_content_buffer = [0u8; TEMPLATE_SIZE];
        loop_content_buffer[..loop_content_length].copy_from_slice(&template[start + start_tag.len()..end]);

        // Clear the loop area in the template
        for i in start..end + end_tag.len() {
            template[i] = 0;
        }

        let mut result_index = start;

        // Iterate over items and replace loop variable with each item
        for item_dict in items {
            let mut loop_iteration = [0; TEMPLATE_SIZE];
            loop_iteration[..loop_content_length].copy_from_slice(&loop_content_buffer[..loop_content_length]);

            for i in 0..item_dict.count {
                if let Some(KeyValuePair { key, value }) = item_dict.entries[i] {
                    let mut placeholder = ByteString::<128>::new(b"{{");
                    placeholder.append(loop_var.as_bytes());
                    placeholder.append(b".");
                    placeholder.append(key.as_bytes());
                    placeholder.append(b"}}");
                    let placeholder_str = core::str::from_utf8(&placeholder.as_bytes()).unwrap_or("");

                    // If format! is not available, construct the placeholder manually
                    replace(&mut loop_iteration, placeholder_str, value);
                }
            }

            // Append the processed loop_iteration back to the template
            for &byte in loop_iteration.iter().take_while(|&&b| b != 0) {
                if result_index < TEMPLATE_SIZE {
                    template[result_index] = byte;
                    result_index += 1;
                }
            }
        }

        // Manually shift the rest of the template accordingly
        let shift_start = result_index;
        let shift_length = TEMPLATE_SIZE - end - end_tag.len();
        for i in 0..shift_length {
            if end + end_tag.len() + i < TEMPLATE_SIZE {
                template[shift_start + i] = template[end + end_tag.len() + i];
            }
        }

        // Zero out the remaining part of the template
        template[shift_start + shift_length..].fill(0);
    }
}


#[macro_export]
macro_rules! include_str_checked {
    ($file:expr, $max_len:expr) => {{
        const CONTENT: &str = include_str!($file);
        const LENGTH: usize = CONTENT.len();

        // Compile-time length check
        let _ = [(); $max_len - LENGTH]; // Will fail if LENGTH > $max_len

        CONTENT
    }};
}

#[cfg(test)]
mod tests {
    use super::*; // Import your http module functions

    #[test]
    fn test_template() { // Template string with full if-else structure
        let template_str = b"Hello, {{name}}. {{#if condition}}Welcome!{{/if}}{{#else}}Goodbye.{{/else}} {{#if exit}}Nope{{/if2}} {{#for person in people}}Name: {{person.name}}, Age: {{person.age}}, {{/for person in people}}";

        // Initialize a buffer with zeros and copy the template string into it
        let mut template: [u8; TEMPLATE_SIZE] = [0; TEMPLATE_SIZE];
        template[..template_str.len()].copy_from_slice(template_str);


        // Replace placeholders
        replace(&mut template, "{{name}}", "Alice");



        // Process if-else condition (true condition in this case)
        process_if(&mut template, true, "{{#if condition}}", "{{/if}}", Some("{{#else}}"), Some("{{/else}}"));

        process_if(&mut template, false, "{{#if exit}}", "{{/if2}}", None, None);

        // Process the for-loop
        let mut dict1 = StaticDict::new();
        dict1.insert("name", "Alice");
        dict1.insert("age", "30");

        let mut dict2 = StaticDict::new();
        dict2.insert("name", "Bob");
        dict2.insert("age", "25");

        let items = [dict1, dict2];
        process_for(&mut template, "person", "people", &items);


        // Convert the buffer to a string and print for verification
        let result = core::str::from_utf8(&template).unwrap_or("<invalid UTF-8>");
        // println!("Actual: {}", result);

        // Assert to check if the result matches expected output
        assert_eq!(result.trim_end_matches(char::from(0)), "Hello, Alice. Welcome!  Name: Alice, Age: 30, Name: Bob, Age: 25, ");
    }
}