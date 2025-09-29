// Automated coordinate system tests
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coordinate_conversions() {
        let testcases = vec![
            // (screen_x, screen_y, viewport_x, viewport_y, expected_doc_x, expected_doc_y)
            (10, 5, 0, 0, 10, 5),      // No scroll
            (10, 5, 5, 0, 15, 5),      // Horizontal scroll
            (10, 5, 0, 10, 10, 15),    // Vertical scroll
            (10, 5, 5, 10, 15, 15),    // Both scrolls
        ];

        for (sx, sy, vx, vy, ex, ey) in testcases {
            let (dx, dy) = screen_to_document(sx, sy, vx, vy);
            assert_eq!((dx, dy), (ex, ey),
                "Failed: screen({},{}) + viewport({},{}) should = doc({},{})",
                sx, sy, vx, vy, ex, ey);
        }
    }

    #[test]
    fn test_roundtrip_conversion() {
        // Test that converting screen→doc→screen gives us back the original
        for x in 0..50 {
            for y in 0..30 {
                let (doc_x, doc_y) = screen_to_document(x, y, 0, 0);
                let (screen_x, screen_y) = document_to_screen(doc_x, doc_y, 0, 0);
                assert_eq!((screen_x, screen_y), (x, y));
            }
        }
    }
}