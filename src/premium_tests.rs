#[cfg(test)]
mod tests {
    use crate::calculate_age;

    #[test]
    fn test_my_function() {
        // Test case logic
        let dob_str = String::from("1977-09-14");
        let result = calculate_age(&dob_str);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 45);
    }
}
