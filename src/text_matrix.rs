pub fn matrix_to_string(matrix: &[Vec<char>]) -> String {
    let mut result = String::new();
    
    for row in matrix {
        for &ch in row {
            result.push(ch);
        }
        // Trim trailing spaces from each line
        let line = result.trim_end();
        result.truncate(line.len());
        result.push('\n');
    }
    
    result
}