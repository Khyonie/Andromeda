macro_rules! interpolate_str {
    ($template:expr, $($name:ident = $value:expr),* $(,)?) => {{
        let substitutions = [$( (concat!(stringify!($name), "}"), $value.to_string()) ),*];
        let mut remaining: &str = $template;
        let mut result = String::new();
        // Only scan the template: braces in a key comment are literal data,
        // never another placeholder to expand.
        while let Some(index) = remaining.find('{') {
            result.push_str(&remaining[..index]);
            remaining = &remaining[index + 1..];
            if let Some((placeholder, value)) = substitutions.iter().find(|(name, _)| remaining.starts_with(name)) {
                result.push_str(value);
                remaining = &remaining[placeholder.len()..];
            } else {
                result.push('{');
            }
        }
        result.push_str(remaining);
        result
    }};
}

pub(crate) use interpolate_str;
