use core::fmt;
use core::fmt::Write;

pub const MAX_HEADERS: usize = 16;
pub const MAX_HEADER_KEY: usize = 32;
pub const MAX_HEADER_VALUE: usize = 128;
pub const MAX_HEADER_VALUES: usize = 4;
pub const BUFFER_SIZE: usize = 1024 * 16;
pub const MAX_URI_LENGTH: usize = 2048;
pub const MAX_METHOD_LENGTH: usize = 20;
pub const MAX_PATH_LENGTH: usize = 1024;
pub const MAX_QUERY_LENGTH: usize = 1024;
pub const MAX_QUERY_PARAMS: usize = 24;
pub const MAX_QUERY_PARAM_LENGTH: usize = 128;
pub const MAX_POST_LENGTH: usize = 1024 * 4;
pub const MAX_POST_PARAMS: usize = 24;
pub const MAX_POST_PARAM_LENGTH: usize = 128;
const MAX_BOUNDARY_COUNT: usize = 32;


#[derive(Copy)]
#[derive(Clone)]
#[derive(PartialEq)]
pub struct ByteString<const N: usize> {
    data: [u8; N],
    length: usize,
}

impl<const N: usize> ByteString<N> {
    pub fn new(data: &[u8]) -> Self {
        let mut byte_string = Self {
            data: [0; N],
            length: 0,
        };

        let copy_len = core::cmp::min(N, data.len());
        byte_string.data[..copy_len].copy_from_slice(&data[..copy_len]);
        byte_string.length = copy_len;

        byte_string
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.data[..self.length]
    }

    pub fn append(&mut self, buffer: &[u8]) {
        let total_space = N;
        let current_length = self.length;
        let space_remaining = total_space - current_length;

        // Determine how much data can be copied
        let amount_to_copy = core::cmp::min(buffer.len(), space_remaining);

        // Calculate the start and end indices for copying
        let start = self.length;
        let end = start + amount_to_copy;

        // Perform the copy
        self.data[start..end].copy_from_slice(&buffer[..amount_to_copy]);

        // Update the length of data in self.data
        self.length += amount_to_copy;
    }

    pub fn len(&self) -> usize {
        self.length
    }

    pub fn split<F>(&self, predicate: F, output: &mut [Option<Self>])
        where
            F: Fn(&u8) -> bool,
    {
        let mut start = 0;
        let mut end = 0;
        let mut index = 0;

        while end < self.length && index < output.len() {
            if predicate(&self.data[end]) {
                if end > start {
                    output[index] = Some(ByteString::new(&self.data[start..end]));
                    index += 1;
                }
                start = end + 1;
            }
            end += 1;
        }

        if end > start && index < output.len() {
            output[index] = Some(ByteString::new(&self.data[start..end]));
        }
    }


    pub fn trim(&mut self) {
        // Find the start index of the trimmed string
        let mut start = 0;
        while start < self.length && (self.data[start] == b' ' || self.data[start] == b'\n' || self.data[start] == b'\r' || self.data[start] == b'"') {
            start += 1;
        }

        // Find the end index of the trimmed string
        let mut end = self.length;
        while end > start && (self.data[end - 1] == b' ' || self.data[end - 1] == b'\n' || self.data[end - 1] == b'\r' || self.data[end - 1] == b'"') {
            end -= 1;
        }

        // Calculate the new length after trimming
        let new_length = end - start;

        // Shift the data to the beginning
        for i in 0..new_length {
            self.data[i] = self.data[start + i];
        }

        // Update the length
        self.length = new_length;
    }
}

// pub fn write_error_to_byte_string<const N: usize>(err: &impl fmt::Debug, byte_string: &mut ByteString<N>) {
//     let args = core::fmt::format(fmt::Arguments::new_v1_formatted(
//         &[""],
//         &match () {
//             () => [core::fmt::ArgumentV1::new(&err, fmt::Debug::fmt)],
//         },
//         &[core::fmt::rt::v1::Argument { position: core::fmt::rt::v1::Position::At(0), format: core::fmt::rt::v1::FormatSpec::new_debug() }],
//     ));
//
//     byte_string.write_fmt(args).unwrap();
// }

impl<const N: usize> Write for ByteString<N> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let bytes = s.as_bytes();
        let bytes_to_write = self.data.len().min(bytes.len());

        self.append(&bytes[..bytes_to_write]);
        Ok(())
    }
    fn write_fmt(&mut self, args: fmt::Arguments<'_>) -> fmt::Result {
        // Using core's write_fmt implementation
        core::fmt::write(self, args)
    }
}


type HeaderValues<const M: usize> = Option<ByteString<M>>;
type Header<const N: usize, const M: usize> = (Option<ByteString<N>>, usize, HeaderValues<M>);


type QueryParams<'a> = [Option<QueryParam<'a>>; MAX_QUERY_PARAMS];

pub struct Headers<const N: usize, const M: usize> {
    pub data: [Header<N, M>; MAX_HEADERS],
}

impl<const N: usize, const M: usize> Headers<N, M> {
    pub fn new() -> Headers<N, M> {
        Headers {
            data: [(None, 0, None); MAX_HEADERS],
        }
    }

    pub fn append(&mut self, key: ByteString<N>, value: Option<ByteString<M>>) {
        let header_values = match value {
            None => create_header_values(&[]),
            Some(val) => create_header_values(&[val]),
        };
        append_header(&mut self.data, key, header_values);
    }
}

#[derive(Debug)]
pub struct QueryParam<'a> {
    pub key: Option<&'a [u8]>,
    pub value: Option<&'a [u8]>,
}

impl<'a> Default for QueryParam<'a> {
    fn default() -> Self {
        QueryParam::new()
    }
}


pub struct Response<const N: usize, const M: usize> {
    pub status: usize,
    pub headers: Headers<N, M>,
    pub body: ByteString<BUFFER_SIZE>,
}


impl<const N: usize, const M: usize> Response<N, M> {
    // Initialize a new Response with generic parameters
    pub fn new() -> Response<N, M> {
        Response {
            status: 404, // Default status, can be changed later
            body: ByteString::<BUFFER_SIZE>::new(&[]),
            headers: Headers::<N, M>::new(),
        }
    }

    pub fn write(&mut self, data: &[u8]) {
        self.body.append(data);
    }

    pub fn header(&mut self, key: ByteString<N>, value: ByteString<M>) {
        self.headers.append(key, Some(value));
    }

    pub fn generate(&mut self) -> ([u8; BUFFER_SIZE], usize) {
        generate_http_response::<N, M>(self.status, &mut self.headers, self.body.as_bytes())
    }
}

pub struct Request {
    pub method: ByteString<MAX_METHOD_LENGTH>,
    pub uri: ByteString<MAX_URI_LENGTH>,
    pub path: ByteString<MAX_PATH_LENGTH>,
    pub query_param_keys: [ByteString<MAX_QUERY_PARAM_LENGTH>; MAX_QUERY_PARAMS],
    pub query_param_values: [ByteString<MAX_QUERY_PARAM_LENGTH>; MAX_QUERY_PARAMS],
    pub query_param_count: usize,
    pub post_param_keys: [ByteString<MAX_POST_PARAM_LENGTH>; MAX_POST_PARAMS],
    pub post_param_values: [ByteString<MAX_POST_PARAM_LENGTH>; MAX_POST_PARAMS],
    pub post_param_count: usize,
    pub headers: Headers<MAX_HEADER_KEY, MAX_HEADER_VALUE>,
}

impl Request {
    pub fn new() -> Request {
        let query_param_keys = [ByteString::new(&[0; MAX_QUERY_PARAM_LENGTH]); MAX_QUERY_PARAMS];
        let query_param_values = [ByteString::new(&[0; MAX_QUERY_PARAM_LENGTH]); MAX_QUERY_PARAMS];
        let post_param_keys = [ByteString::new(&[0; MAX_POST_PARAM_LENGTH]); MAX_POST_PARAMS];
        let post_param_values = [ByteString::new(&[0; MAX_POST_PARAM_LENGTH]); MAX_POST_PARAMS];

        Request {
            method: ByteString::new(&[0; MAX_METHOD_LENGTH]),
            uri: ByteString::new(&[0; MAX_URI_LENGTH]),
            path: ByteString::new(&[0; MAX_PATH_LENGTH]),
            query_param_keys,
            query_param_values,
            query_param_count: 0,
            post_param_keys,
            post_param_values,
            post_param_count: 0,
            headers: Headers::new(),
        }
    }

    pub(crate) fn parse(&mut self, buf: &[u8], n: usize) {
        self.headers.data = parse_http_headers(buf, n, false);

        if !self.headers.data.is_empty() {
            // Assuming headers[0].0 contains the request line
            if let Some(key) = self.headers.data[0].0 {
                if let Some((method, uri)) = parse_request_line(key.as_bytes()) {
                    // Process method and uri immediately within the scope
                    self.method = ByteString::new(method);
                    self.uri = ByteString::new(uri);

                    let (path, params) = split_path_and_query(&self.uri);
                    self.path = path;
                    self.query_param_count = parse_query_string(&params, &mut self.query_param_keys, &mut self.query_param_values);
                } else {
                    // Handle the case where parse_request_line returns None
                    // This might involve setting default values, logging an error, etc.
                }
            }


            if self.method.as_bytes() == b"POST" {
                let (content_type, boundary): (ByteString<128>, ByteString<128>) = match get_header(self.headers.data, b"Content-Type") {
                    None => (ByteString::new(b""), ByteString::new(b"")),
                    Some(content_types) => {
                        let content_type = content_types.unwrap_or_else(|| ByteString::new(b""));

                        if content_type.as_bytes() == b"application/x-www-form-urlencoded" {
                            (content_type, ByteString::new(b""))
                        } else {
                            let mut content_type_pairs: [Option<ByteString<128>>; 2] = Default::default();
                            content_type.split(|&b| b == b';', &mut content_type_pairs);

                            if let [Some(lead), Some(mut trail)] = &content_type_pairs {
                                trail.trim();

                                let mut boundary_pairs: [Option<ByteString<128>>; 2] = Default::default();
                                trail.split(|&b| b == b'=', &mut boundary_pairs);

                                if let [Some(_), Some(boundary_value)] = &boundary_pairs {
                                    (lead.clone(), boundary_value.clone())
                                } else {
                                    (ByteString::new(b""), ByteString::new(b""))
                                }
                            } else {
                                // Your code for when there are not enough pairs
                                (ByteString::new(b""), ByteString::new(b""))
                            }
                        }
                    }
                };


                match content_type.as_bytes() {
                    b"application/x-www-form-urlencoded" => {
                        self.parse_post_url_encoded(&buf);
                    }
                    b"multipart/form-data" => {
                        // Grab Content Length
                        let content_length = match get_header(self.headers.data, b"Content-Length") {
                            Some(content_lengths) => {
                                let content_length = match parse_bytes_to_usize(content_lengths.unwrap_or_else(|| ByteString::new(b"0")).as_bytes()) {
                                    Some(length) => length,
                                    _ => 0,
                                };
                                content_length
                            }
                            _ => 0
                        };

                        // Calculate the start index for the slice
                        let start_index = if content_length > buf.len() {
                            0 // If Content-Length is greater than the buffer size, start from the beginning
                        } else {
                            buf.len() - content_length // Otherwise, start from buf.len() - content_length
                        };

                        // let multipart_buff = &buf[start_index..];

                        let mut delimited_boundary = ByteString::<128>::new(b"--");
                        delimited_boundary.append(boundary.as_bytes());

                        let mut delimited_boundary_end = ByteString::<128>::new(b"--");
                        delimited_boundary_end.append(boundary.as_bytes());
                        delimited_boundary_end.append(b"--");

                        let delimited_boundary_end_str = core::str::from_utf8(delimited_boundary_end.as_bytes())
                            .expect("Invalid UTF-8");

                        let end_pos = match core::str::from_utf8(buf).expect("Invalid UTF-8")
                            .rfind(delimited_boundary_end_str)
                        {
                            Some(pos) => pos,
                            None => buf.len(), // If not found, assume it's the end of the buffer
                        };

                        let multipart_buff = &buf[start_index..end_pos];

                        // println!("multipart_buff:\n{}", core::str::from_utf8(&multipart_buff[..multipart_buff.len()]).unwrap_or("<invalid UTF-8>"));

                        // println!("delimited_boundary:\n{}", core::str::from_utf8(&delimited_boundary.as_bytes()).unwrap_or("<invalid UTF-8>"));
                        let positions = find_boundary_positions(multipart_buff, delimited_boundary.as_bytes());

                        for (index, &pos) in positions.iter().enumerate() {
                            if index > 0 && pos == 0 {
                                // If the current position is zero, it indicates the end of boundaries
                                break;
                            }

                            // println!("Boundary {} found at position: {}", index + 1, pos);

                            // Calculate the end position for the current boundary
                            let end_pos = if index + 1 < positions.len() && positions[index + 1] > 0 {
                                positions[index + 1]
                            } else {
                                multipart_buff.len() // Assume it's the end if there is no next position
                            };

                            // Get a slice of the buffer to parse inside the boundary
                            let boundary_slice: &[u8] = &multipart_buff[(pos + delimited_boundary.len() + 2)..end_pos];

                            let separator = b"\r\n\r\n";

                            if let Some(index) = boundary_slice.windows(separator.len()).position(|window| window == separator) {
                                // Found the position of \r\n\r\n
                                let first_slice = &boundary_slice[0..index + 2];
                                let second_slice = &boundary_slice[index + separator.len()..boundary_slice.len() - 2];

                                let boundary_headers: [Header<32, 128>; 16] = parse_http_headers(first_slice, first_slice.len(), true);

                                match get_header(boundary_headers, b"Content-Disposition") {
                                    None => {
                                        // println!("Content Disposition not found!");
                                    }
                                    Some(dispositions) => {
                                        let disposition = dispositions.unwrap_or_else(|| ByteString::new(b""));
                                        let disposition_bytes = disposition.as_bytes();

                                        // println!("disposition:\n'{}'", core::str::from_utf8(disposition.as_bytes()).unwrap_or("<invalid UTF-8>"));


                                        let mut form_data_pairs: [Option<ByteString<128>>; 2] = Default::default();
                                        disposition.split(|&b| b == b';', &mut form_data_pairs);


                                        if let [Some(_), Some(mut name)] = &form_data_pairs {
                                            let mut value_pairs: [Option<ByteString<128>>; 2] = Default::default();
                                            name.split(|&b| b == b'=', &mut value_pairs);

                                            if let [Some(_), Some(mut form_data_name)] = &value_pairs {
                                                // println!("form_data_name:\n'{}'", core::str::from_utf8(form_data_name.as_bytes()).unwrap_or("<invalid UTF-8>"));

                                                let clean_name = trim_quotes(form_data_name.as_bytes());

                                                // println!("form_data_clean_name:\n'{}'", core::str::from_utf8(clean_name).unwrap_or("<invalid UTF-8>"));


                                                self.post_param_keys[self.post_param_count] = ByteString::new(clean_name);
                                                self.post_param_values[self.post_param_count] = ByteString::new(second_slice);
                                                self.post_param_count += 1;
                                            } else {
                                            }
                                        } else {
                                        }
                                    }
                                }
                            } else {
                                // \r\n\r\n not found in the byte array
                                // Handle this case accordingly
                            }
                            // parse_http_headers

                            // println!("Boundary Buffer:\n{}", core::str::from_utf8(&boundary_slice[..boundary_slice.len()]).unwrap_or("<invalid UTF-8>"));
                        }


                    }
                    _ => {}
                }



            }
        }
    }

    fn parse_post_url_encoded(&mut self, buf: &&[u8]) {
        let post_content_length = self.headers.data.iter()
            .find_map(|(key_option, _, values)| {
                key_option.as_ref().filter(|key| key.as_bytes() == b"Content-Length")
                    .and_then(|_| values.as_ref())
                    .map(|value| value.as_bytes())
            })
            .and_then(parse_bytes_to_usize)
            .unwrap_or(0);

        if buf.len() >= post_content_length {
            let post_data = ByteString::new(&buf[buf.len() - post_content_length..]);
            self.post_param_count = parse_query_string(&post_data, &mut self.post_param_keys, &mut self.post_param_values);
        } else {
            // Handle the error case where the buffer is too short
        }
    }

    pub fn get(&self, key: &[u8]) -> Option<&[u8]> {
        get_query_param_value(self.query_param_count, &self.query_param_keys, &self.query_param_values, key)
    }

    pub fn post(&self, key: &[u8]) -> Option<&[u8]> {
        get_query_param_value(self.post_param_count, &self.post_param_keys, &self.post_param_values, &key)
    }
}

impl<'a> QueryParam<'a> {
    pub fn new() -> QueryParam<'a> {
        QueryParam {
            key: None,
            value: None,
        }
    }

    // Methods to set the key and value
    // ...
}

fn get_status_message(status_code: usize) -> &'static str {
    match status_code {
        200 => "OK",
        404 => "Not Found",
        500 => "Internal Server Error",
        // Add other status codes as needed
        _ => "Unknown",
    }
}

fn trim_quotes(input: &[u8]) -> &[u8] {
    if input.starts_with(b"\"") && input.ends_with(b"\"") && input.len() >= 2 {
        &input[1..input.len() - 1]
    } else {
        input
    }
}

fn find_boundary_positions(buf: &[u8], boundary: &[u8]) -> [usize; MAX_BOUNDARY_COUNT] {

    let mut positions = [0; MAX_BOUNDARY_COUNT];
    let mut count = 0;
    let mut start = 0;

    while let Some(pos) = buf[start..].windows(boundary.len()).position(|window| window == boundary) {
        // Adjust the position to the absolute position in the original buffer
        let absolute_pos = start + pos;
        positions[count] = absolute_pos;
        count += 1;

        // Move the start position to the character after the found boundary
        start = absolute_pos + boundary.len();

        if count >= MAX_BOUNDARY_COUNT {
            break; // Stop searching after reaching the maximum count
        }
    }

    positions
}


pub fn convert_slice_to_fixed_array<const SIZE: usize>(slice: &[u8]) -> [u8; SIZE] {
    let mut array = [0; SIZE];
    let length = slice.len().min(SIZE);
    array[..length].copy_from_slice(&slice[..length]);
    array
}

pub fn trim_bytes(bytes: &[u8]) -> &[u8] {
    let mut start = 0;
    let mut end = bytes.len();

    // Trim leading spaces
    for (index, &byte) in bytes.iter().enumerate() {
        if byte != b' ' && byte != b'\t' { // checking for space and tab
            start = index;
            break;
        }
    }

    // Trim trailing spaces
    for (index, &byte) in bytes.iter().enumerate().rev() {
        if byte != b' ' && byte != b'\t' {
            end = index + 1;
            break;
        }
    }

    &bytes[start..end]
}

pub fn usize_to_bytes(value: usize, buffer: &mut [u8]) -> usize {
    let mut n = value;
    let mut len = 0;

    // Calculate the number of digits in the value
    while n > 0 {
        n /= 10;
        len += 1;
    }

    let mut idx = len;
    let mut mutable_value = value;

    // Convert the value to bytes as ASCII characters
    while mutable_value > 0 {
        idx -= 1;
        let digit = (mutable_value % 10) as u8;
        buffer[idx] = b'0' + digit;
        mutable_value /= 10;
    }

    len
}

// GET / HTTP/1.1
// Host: 192.168.3.181:8000
// Connection: keep-alive
// Upgrade-Insecure-Requests: 1
// User-Agent: Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/119.0.0.0 Safari/537.36
// Accept: text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7
// Accept-Encoding: gzip, deflate
// Accept-Language: en-US,en;q=0.9
pub fn parse_request_line(buf: &[u8]) -> Option<(&[u8], &[u8])> {
    let mut parts = buf.split(|&b| b == b' ');

    if let (Some(method), Some(path)) = (parts.next(), parts.next()) {
        Some((method, path))
    } else {
        None
    }
}

pub fn generate_http_response<const N: usize, const M: usize>(
    status_code: usize,
    headers: &mut Headers<N, M>,
    body: &[u8]
) -> ([u8; BUFFER_SIZE], usize) {
    let mut response = ByteString::<BUFFER_SIZE>::new(b"HTTP/1.1 ");

    // Append status code and status message
    let status_message = get_status_message(status_code);
    let mut status_code_bytes: [u8; 3] = Default::default();

    usize_to_bytes(status_code, &mut status_code_bytes);

    response.append(&status_code_bytes);
    response.append(b" ");
    response.append(status_message.as_bytes());
    response.append(b"\r\n");

    // Convert content length to bytes and create Content-Length header
    let content_length = body.len();
    let mut content_length_bytes = [0u8; 10];
    let content_length_len = usize_to_bytes(content_length, &mut content_length_bytes);
    let content_length_key = ByteString::<N>::new(b"Content-Length");
    let content_length_value_bytes = ByteString::<M>::new(&content_length_bytes[..content_length_len]);
    let content_length_value: HeaderValues<M> = Some(content_length_value_bytes);
    append_header(&mut headers.data, content_length_key, content_length_value);

    // Append headers
    match combine_headers(&mut headers.data, &mut response.data, response.length) {
        Ok(size) => {
            response.length = size;
        },
        Err(_) => return (response.data, 0), // Handle error if headers don't fit
    };

    // Append blank line and body
    response.append(b"\r\n");
    response.append(body);

    (response.data, response.length)
}

pub fn parse_http_headers<const N: usize, const M: usize>(
    buf: &[u8],
    n: usize,
    include_first_line: bool,
) -> [Header<N, M>; MAX_HEADERS] {
    let mut headers = [None; MAX_HEADERS];
    let mut header_index = 0;
    let mut i = 0;

    while i < n && header_index < MAX_HEADERS {
        let mut line_end = None;
        for j in i..n {
            if j + 1 < n && buf[j] == b'\r' && buf[j + 1] == b'\n' {
                line_end = Some(j);
                break;
            }
        }

        if let Some(end) = line_end {
            let line = &buf[i..end];

            if header_index == 0 && !include_first_line {
                // Convert the first line to ByteString and set as the first header
                let request_line = ByteString::<N>::new(line);
                headers[header_index] = Some((request_line, 1, None));
                header_index += 1;
            } else {
                if let Some(colon_index) = line.iter().position(|&b| b == b':') {
                    let key_slice = trim_bytes(&line[..colon_index]);
                    let value_slice = trim_bytes(&line[colon_index + 1..]);

                    let key = ByteString::<N>::new(key_slice);
                    let value = ByteString::<M>::new(value_slice);

                    headers[header_index] = Some((key, 1, Some(value)));
                    header_index += 1;
                }
            }
            i = end + 2;
        } else {
            break;
        }
    }

    // Convert the Option<Header> array to a fixed-size array
    let mut result = [(None, 0, None); MAX_HEADERS];
    for (index, header) in headers.iter().enumerate() {
        if let Some((key, count, value)) = header {
            result[index] = (Some(key.clone()), *count, value.clone());
        }
    }

    result
}



pub fn get_header<const N: usize, const M: usize>(
    headers: [Header<N, M>; MAX_HEADERS],
    header_name: &[u8]
) -> Option<HeaderValues<M>> {
    for header in headers.iter() {
        if let (Some(key), count, values) = header {
            if key.as_bytes() == header_name && *count > 0 {
                return Some(*values);
            }
        }
    }
    None
}

pub fn parse_query_string<'a>(
    query: &'a ByteString<MAX_QUERY_LENGTH>,
    query_param_keys: &mut [ByteString<MAX_QUERY_PARAM_LENGTH>; MAX_QUERY_PARAMS],
    query_param_values: &mut [ByteString<MAX_QUERY_PARAM_LENGTH>; MAX_QUERY_PARAMS],
) -> usize {
    let mut param_index = 0;

    let mut pairs: [Option<ByteString<MAX_QUERY_LENGTH>>; MAX_QUERY_PARAMS] = [None; MAX_QUERY_PARAMS];
    query.split(|&b| b == b'&', &mut pairs);

    for pair_option in pairs.iter() {
        if let Some(pair) = pair_option {
            if param_index >= MAX_QUERY_PARAMS {
                break;
            }

            let mut key_value_pair: [Option<ByteString<MAX_QUERY_LENGTH>>; 2] = [None; 2];
            pair.split(|&b| b == b'=', key_value_pair.as_mut_slice()); // Adjusted this line

            if let (Some(key), Some(value)) = (key_value_pair[0].as_ref(), key_value_pair[1].as_ref()) {
                query_param_keys[param_index] = ByteString::<MAX_QUERY_PARAM_LENGTH>::new(key.as_bytes());
                query_param_values[param_index] = ByteString::<MAX_QUERY_PARAM_LENGTH>::new(value.as_bytes());
                param_index += 1;
            }
        }
    }

    param_index
}


pub fn get_query_param_value<'a>(
    query_param_count: usize,
    query_param_keys: &'a [ByteString<MAX_QUERY_PARAM_LENGTH>; MAX_QUERY_PARAMS],
    query_param_values: &'a [ByteString<MAX_QUERY_PARAM_LENGTH>; MAX_QUERY_PARAMS],
    key_to_find: &[u8],
) -> Option<&'a [u8]> {
    for i in 0..query_param_count {
        let key = &query_param_keys[i];
        let value = &query_param_values[i];

        if key.as_bytes() == key_to_find {
            return Some(value.as_bytes());
        }
    }
    None
}

pub fn split_path_and_query(path: &ByteString<MAX_URI_LENGTH>) -> (ByteString<MAX_PATH_LENGTH>, ByteString<MAX_QUERY_LENGTH>) {
    if let Some(index) = path.as_bytes().iter().position(|&b| b == b'?') {
        let (path_slice, query_slice) = path.as_bytes().split_at(index);
        let query = query_slice.get(1..).unwrap_or(&[]);
        (
            ByteString::new(path_slice),
            ByteString::new(query)
        )
    } else {
        (
            ByteString::new(path.as_bytes()), // Create a new ByteString<MAX_PATH_LENGTH>
            ByteString::new(b"")
        )
    }
}



pub fn append_header<const N: usize, const M: usize>(
    headers: &mut [Header<N, M>; MAX_HEADERS],
    key: ByteString<N>,
    value: HeaderValues<M>,
) {
    for header in headers.iter_mut() {
        if header.0.is_none() {
            // Found an empty slot
            *header = (Some(key), 1, value);
            return;
        }
    }
}

pub fn create_header_values<const N: usize>(values: &[ByteString<N>]) -> HeaderValues<N> {
    let mut header_value: Option<ByteString<N>> = None;
    for (i, &value) in values.iter().enumerate() {
        if i >= MAX_HEADER_VALUES {
            break; // Avoid exceeding the array size
        }
        header_value = Some(value);
    }
    header_value
}

fn combine_headers<const N: usize, const M: usize>(
    headers: &[Header<N, M>],
    buffer: &mut [u8],
    offset: usize,  // Starting offset in the buffer
) -> Result<usize, &'static str> {
    let mut cursor = offset;  // Start from the provided offset

    for &(ref key, _, ref value) in headers.iter() {
        if let Some(ref key) = key {
            let key_bytes = key.as_bytes();
            if cursor + key_bytes.len() + 4 > buffer.len() {
                return Err("Buffer too small");
            }

            buffer[cursor..cursor + key_bytes.len()].copy_from_slice(key_bytes);
            cursor += key_bytes.len();

            if let Some(ref value) = value {
                let value_bytes = value.as_bytes();
                if cursor + value_bytes.len() + 2 > buffer.len() {
                    return Err("Buffer too small");
                }

                buffer[cursor..cursor + 2].copy_from_slice(b": ");
                cursor += 2;

                buffer[cursor..cursor + value_bytes.len()].copy_from_slice(value_bytes);
                cursor += value_bytes.len();
            }

            buffer[cursor..cursor + 2].copy_from_slice(b"\r\n");
            cursor += 2;
        }
    }

    Ok(cursor) // Return the new offset
}

pub fn bytes_to_readable_string(bytes: &[u8], output: &mut [u8]) {
    for (i, &byte) in bytes.iter().enumerate() {
        if i >= output.len() {
            break; // Avoid exceeding the buffer size
        }

        // Check if byte is printable ASCII (space to tilde)
        if (byte >= 0x20 && byte <= 0x7E) || byte == b'\n' || byte == b'\r' || byte == b'\t' {
            output[i] = byte;
        } else {
            output[i] = b'.'; // Non-printable characters are replaced with a dot
        }
    }
}

fn parse_bytes_to_usize(bytes: &[u8]) -> Option<usize> {
    let mut num = 0;
    for &byte in bytes {
        if byte >= b'0' && byte <= b'9' {
            num = num * 10 + (byte - b'0') as usize;
        } else {
            // Return None if the byte slice contains non-digit characters
            return None;
        }
    }
    Some(num)
}


#[cfg(test)]
mod tests {
    use super::*; // Import your http module functions

    #[test]
    fn test_parse_http_headers() {
        let buf = b"GET / HTTP/1.1\r\nHost: 192.168.3.181:8000\r\nConnection: keep-alive\r\n..."; // rest of the buffer
        let headers: [(Option<ByteString<MAX_HEADER_KEY>>, usize, Option<ByteString<MAX_HEADER_VALUE>>); MAX_HEADERS] = parse_http_headers(buf, buf.len(), false);

        assert_eq!(headers.len(), MAX_HEADERS, "Expected {} headers, found {}", MAX_HEADERS, headers.len());

        if !headers.is_empty() {
            if let Some(key) = headers[0].0 {
                if let Some((method, path)) = parse_request_line(key.as_bytes()) {
                    println!("Method: {:?}, Path: {:?}", method, path);

                    // Assert conditions directly within the scope
                    assert_eq!(method, b"GET", "Expected GET, found {:?}", method);
                    assert_eq!(path, b"/", "Expected /, found {:?}", path);
                } else {
                    panic!("parse_request_line returned None");
                }
            } else {
                panic!("First header key is None");
            }
        } else {
            panic!("Headers are empty");
        }
    }

    #[test]
    fn test_parse_uri() {
        let buf = b"GET /sign-up?foo=123&bar=22 HTTP/1.1\r\nHost: 192.168.3.181:8000\r\n..."; // rest of the buffer
        let headers: [(Option<ByteString<MAX_HEADER_KEY>>, usize, Option<ByteString<MAX_HEADER_VALUE>>); MAX_HEADERS] = parse_http_headers(buf, buf.len(), false);

        if !headers.is_empty() {
            if let Some(key) = headers[0].0 {
                if let Some((method, path)) = parse_request_line(key.as_bytes()) {
                    assert_eq!(method, b"GET", "Expected GET, found {:?}", method);
                    assert_eq!(path, b"/sign-up?foo=123&bar=22", "Expected /sign-up?foo=123&bar=22, found {:?}", path);
                } else {
                    panic!("parse_request_line returned None");
                }
            } else {
                panic!("First header key is None");
            }
        } else {
            panic!("Headers are empty");
        }
    }

    #[test]
    fn test_parse_query_string() {
        let uri = ByteString::<MAX_URI_LENGTH>::new(b"/sign-up?foo=123&bar=22");
        let (path, params) = split_path_and_query(&uri);

        let mut query_param_keys = [ByteString::new(&[0; MAX_QUERY_PARAM_LENGTH]); MAX_QUERY_PARAMS];
        let mut query_param_values = [ByteString::new(&[0; MAX_QUERY_PARAM_LENGTH]); MAX_QUERY_PARAMS];

        let query_param_count = parse_query_string(&params, &mut query_param_keys, &mut query_param_values);

        for i in 0..query_param_count {
            let key:&ByteString<MAX_QUERY_PARAM_LENGTH> = &query_param_keys[i];
            let value:&ByteString<MAX_QUERY_PARAM_LENGTH> = &query_param_values[i];

            let key_str = key.as_bytes();
            let value_str = value.as_bytes();

            if key_str == b"foo" {
                assert_eq!(value_str, b"123", "Expected value '123' for key 'foo', found {:?}", value_str);
            } else if key_str == b"bar" {
                assert_eq!(value_str, b"22", "Expected value '22' for key 'bar', found {:?}", value_str);
            } else {
                panic!("Unexpected key: {:?}", key_str);
            }
        }

        let foo_value = get_query_param_value(query_param_count, &query_param_keys, &query_param_values, b"foo");
        assert_eq!(foo_value, Some(&b"123"[..]), "Expected value for 'foo' is 123");

        let bar_value = get_query_param_value(query_param_count, &query_param_keys, &query_param_values, b"bar");
        assert_eq!(bar_value, Some(&b"22"[..]), "Expected value for 'bar' is 22");
    }

    #[test]
    fn test_response() {
        let mut content_length_bytes = [0u8; 10];

        let mut headers = Headers::<MAX_HEADER_KEY, MAX_HEADER_VALUE>::new();
        headers.append(ByteString::new(b"Content-Type"),  Some(ByteString::new(b"text/html")));

        let (http_response, mut response_length) =
            generate_http_response(
                200,
                &mut headers,
                b"hello"
            );

        // Convert to readable format for debugging
        let mut readable_response = [0u8; BUFFER_SIZE];
        bytes_to_readable_string(&http_response[..response_length], &mut readable_response);
        let mut readable_expected = [0u8; BUFFER_SIZE];
        let expected_response: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: 5\r\n\r\nhello";
        bytes_to_readable_string(expected_response, &mut readable_expected);

        // Print or log for debugging
        println!("Actual: {}", core::str::from_utf8(&readable_response[..response_length]).unwrap_or("<invalid UTF-8>"));
        println!("Expected: {}", core::str::from_utf8(&readable_expected[..expected_response.len()]).unwrap_or("<invalid UTF-8>"));

        assert_eq!(&http_response[..response_length], expected_response, "Response did not match expected value.");

        // assert_eq!(http_response[..response_length],
        //            b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: 5\r\n\r\nhello"[..],
        //            "Expected value HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\nhello"
        // );

        let mut buffer: [u8; BUFFER_SIZE] = [0; BUFFER_SIZE];

        match combine_headers(&headers.data, &mut buffer, 0) {
            Ok(size) => {
                response_length += size;
            }
            Err(_) => {
                // Buffer was too small, handle the error
            }
        }
    }


    #[test]
    fn test_request() {
        let buf = b"GET / HTTP/1.1\r\nHost: 192.168.3.181:8000\r\nConnection: keep-alive\r\nUpgrade-Insecure-Requests: 1\r\nUser-Agent: Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/119.0.0.0 Safari/537.36\r\nAccept: text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7\r\nAccept-Encoding: gzip, deflate\r\nAccept-Language: en-US,en;q=0.9\r\n";

        let mut req = Request::new();

        req.parse(buf, 421);


        assert_eq!(&req.method.as_bytes(), b"GET", "Method not GET");
    }

    #[test]
    fn test_http_post() {
        let buf = b"POST /test HTTP/1.1\r\nHost: foo.example\r\nContent-Type: application/x-www-form-urlencoded\r\nContent-Length: 27\r\n\r\nfield1=value1&field2=value2";

        let mut req = Request::new();

        req.parse(buf, 138);

        let field1 = req.post(b"field1");
        let field2 = req.post(b"field2");

        match field1 {
            Some(bytes) => assert_eq!(bytes, b"value1", "POST field != value1"),
            None => panic!("req.post(b\"field1\") is None"),
        }

        match field2 {
            Some(bytes) => assert_eq!(bytes, b"value2", "POST field2 != value2"),
            None => panic!("req.post(b\"field2\") is None"),
        }
    }

    #[test]
    fn test_http_post_multipart() {
        let buf = b"POST /api/upload HTTP/1.1\r\nContent-Length: 242\r\nContent-Type: multipart/form-data; boundary=PieBoundary123456789012345678901234567\r\nHost: localhost:8000\r\nUser-Agent: HTTPie\r\n\r\n--PieBoundary123456789012345678901234567\r\nContent-Disposition: form-data; name=\"field1\"\r\n\r\nvalue1\r\n--PieBoundary123456789012345678901234567\r\nContent-Disposition: form-data; name=\"field2\"\r\n\r\nvalue2\r\n--PieBoundary123456789012345678901234567--\r\n";

        let mut req = Request::new();

        req.parse(buf, 255);

        let field1 = req.post(b"field1");
        let field2 = req.post(b"field2");

        match field1 {
            Some(bytes) => {
                println!("field1:\n'{}'", core::str::from_utf8(bytes).unwrap_or("<invalid UTF-8>"));
                assert_eq!(bytes, b"value1", "POST field != value1")
            },
            None => panic!("req.post(b\"field1\") is None"),
        }

        match field2 {
            Some(bytes) => {
                println!("field2:\n'{}'", core::str::from_utf8(bytes).unwrap_or("<invalid UTF-8>"));
                assert_eq!(bytes, b"value2", "POST field2 != value2")
            },
            None => panic!("req.post(b\"field2\") is None"),
        }
    }
}