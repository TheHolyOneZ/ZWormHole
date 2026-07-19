

const ALPHABET: &[u8] = b"23456789ABCDEFGHJKMNPQRSTUVWXYZ";


pub fn generate_code() -> String {
    let part = |n: usize| -> String {
        (0..n)
            .map(|_| ALPHABET[rand::random_range(0..ALPHABET.len())] as char)
            .collect()
    };
    format!("{}-{}", part(3), part(3))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_expected_shape() {
        let code = generate_code();
        assert_eq!(code.len(), 7);
        assert_eq!(code.chars().nth(3), Some('-'));
        for c in code.chars().filter(|c| *c != '-') {
            assert!(ALPHABET.contains(&(c as u8)), "unexpected char {c}");
        }
    }

    #[test]
    fn is_not_trivially_constant() {
        let a = generate_code();
        let b = generate_code();
        assert_ne!(a, b, "two consecutive codes collided — check RNG source");
    }
}
