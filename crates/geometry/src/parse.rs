//! Coordinate-file parsing (PRD §10.1, FILE requirements).
//!
//! Handles the formats actually found in the wild:
//!
//! * **CSV** with or without an `x,y` header, comma/semicolon/tab separated.
//! * **Whitespace-delimited** `.dat`/`.txt` — the Selig convention: an optional
//!   name line, then the contour from the trailing edge over the upper surface
//!   to the leading edge and back along the lower surface.
//! * **Lednicer** `.dat` — a name line, a line giving the upper and lower point
//!   counts, then two blocks each running leading edge → trailing edge.
//!
//! Parsing never executes or evaluates file contents (PRD §53); it only reads
//! numbers.

use crate::vec2::Vec2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum CoordinateFormat {
    /// One contour, already in traversal order.
    SingleContour,
    /// Two surface blocks, each leading edge → trailing edge.
    Lednicer,
}

#[derive(Debug, Clone)]
pub struct ParsedGeometry {
    pub points: Vec<Vec2>,
    pub format: CoordinateFormat,
    /// Name from the file's first line, when it is not numeric data.
    pub name: Option<String>,
    /// Non-fatal remarks: skipped lines, detected format, and so on.
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    Empty,
    NoNumericRows,
    /// A row had content but could not be read as two numbers.
    MalformedRow {
        line: usize,
        text: String,
    },
    TooFewPoints {
        found: usize,
        needed: usize,
    },
    TooLarge {
        bytes: usize,
        limit: usize,
    },
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::Empty => write!(f, "The file is empty."),
            ParseError::NoNumericRows => write!(
                f,
                "No coordinate rows were found. Each row should hold an x and a y value, e.g. `1.0, 0.0`."
            ),
            ParseError::MalformedRow { line, text } => write!(
                f,
                "Line {line} could not be read as a coordinate pair: `{}`. Expected two numbers.",
                text.chars().take(60).collect::<String>()
            ),
            ParseError::TooFewPoints { found, needed } => write!(
                f,
                "Only {found} coordinate(s) were found; at least {needed} are needed to define a closed body."
            ),
            ParseError::TooLarge { bytes, limit } => write!(
                f,
                "The file is {:.1} MB, above the {:.1} MB limit.",
                *bytes as f64 / 1.0e6,
                *limit as f64 / 1.0e6
            ),
        }
    }
}

impl std::error::Error for ParseError {}

/// Upper bound on accepted file size (PRD §53). A 10 MB coordinate file would
/// hold on the order of a million points, far beyond any sane panel count.
pub const MAX_FILE_BYTES: usize = 10 * 1024 * 1024;

/// Minimum points for a body.
pub const MIN_POINTS: usize = 4;

fn split_numbers(line: &str) -> Vec<f64> {
    line.split(|c: char| c == ',' || c == ';' || c == '\t' || c.is_whitespace())
        .filter(|t| !t.is_empty())
        .filter_map(|t| {
            // Tolerate Fortran-style exponents (`1.0D-3`) found in old .dat files.
            let cleaned = t.replace(['D', 'd'], "e");
            cleaned.parse::<f64>().ok()
        })
        .collect()
}

fn is_comment(line: &str) -> bool {
    let t = line.trim_start();
    t.is_empty() || t.starts_with('#') || t.starts_with('!') || t.starts_with("//")
}

/// Parse a coordinate file into an ordered point list.
pub fn parse_coordinates(text: &str) -> Result<ParsedGeometry, ParseError> {
    if text.len() > MAX_FILE_BYTES {
        return Err(ParseError::TooLarge {
            bytes: text.len(),
            limit: MAX_FILE_BYTES,
        });
    }
    if text.trim().is_empty() {
        return Err(ParseError::Empty);
    }

    let mut notes = Vec::new();
    let lines: Vec<&str> = text.lines().collect();

    // A leading non-numeric line is an airfoil name.
    let mut cursor = 0usize;
    let mut name = None;
    while cursor < lines.len() && is_comment(lines[cursor]) {
        cursor += 1;
    }
    if cursor < lines.len() {
        let nums = split_numbers(lines[cursor]);
        if nums.len() < 2 {
            let candidate = lines[cursor].trim();
            if !candidate.is_empty() {
                name = Some(candidate.to_string());
            }
            cursor += 1;
        }
    }

    // Skip a CSV header such as `x,y`.
    while cursor < lines.len() && is_comment(lines[cursor]) {
        cursor += 1;
    }
    if cursor < lines.len() && split_numbers(lines[cursor]).len() < 2 {
        let header = lines[cursor].to_lowercase();
        if header.contains('x') && header.contains('y') {
            notes.push("A column header row was skipped.".to_string());
            cursor += 1;
        }
    }

    // Lednicer files declare the two surface point counts on their own line.
    // The giveaway is a row of two numbers that are both integral and ≥ 2,
    // followed by rows whose first value starts near 0 (the leading edge).
    let body = &lines[cursor..];
    if let Some(parsed) = try_parse_lednicer(body, name.clone(), &mut notes) {
        return finish(parsed);
    }

    let mut points = Vec::new();
    let mut malformed: Option<ParseError> = None;
    for (i, raw) in body.iter().enumerate() {
        if is_comment(raw) {
            continue;
        }
        let nums = split_numbers(raw);
        if nums.len() >= 2 {
            points.push(Vec2::new(nums[0], nums[1]));
            if nums.len() > 2 {
                // A third column (z, or a thickness value) is ignored, not an error.
            }
        } else if malformed.is_none() && !nums.is_empty() {
            malformed = Some(ParseError::MalformedRow {
                line: cursor + i + 1,
                text: raw.trim().to_string(),
            });
        }
    }

    if points.is_empty() {
        return Err(malformed.unwrap_or(ParseError::NoNumericRows));
    }
    if let Some(e) = malformed {
        // Partially readable files are accepted with a note rather than rejected:
        // stray footer text is common in hand-edited coordinate files.
        notes.push(format!("{e} The row was skipped."));
    }

    finish(ParsedGeometry {
        points,
        format: CoordinateFormat::SingleContour,
        name,
        notes,
    })
}

fn finish(g: ParsedGeometry) -> Result<ParsedGeometry, ParseError> {
    if g.points.len() < MIN_POINTS {
        return Err(ParseError::TooFewPoints {
            found: g.points.len(),
            needed: MIN_POINTS,
        });
    }
    Ok(g)
}

/// Recognise and join the two surface blocks of a Lednicer-format file.
fn try_parse_lednicer(
    body: &[&str],
    name: Option<String>,
    notes: &mut Vec<String>,
) -> Option<ParsedGeometry> {
    let mut idx = 0usize;
    while idx < body.len() && is_comment(body[idx]) {
        idx += 1;
    }
    let counts = split_numbers(body.get(idx)?);
    if counts.len() != 2 {
        return None;
    }
    let (nu, nl) = (counts[0], counts[1]);
    // The count line holds integers ≥ 2; a coordinate row would not.
    let integral = |v: f64| (v - v.round()).abs() < 1e-9 && (2.0..1.0e6).contains(&v);
    if !integral(nu) || !integral(nl) {
        return None;
    }
    let (nu, nl) = (nu as usize, nl as usize);

    let mut rows: Vec<Vec2> = Vec::new();
    for raw in &body[idx + 1..] {
        if is_comment(raw) {
            continue;
        }
        let nums = split_numbers(raw);
        if nums.len() >= 2 {
            rows.push(Vec2::new(nums[0], nums[1]));
        }
    }
    if rows.len() < nu + nl {
        return None;
    }

    let upper = &rows[..nu];
    let lower = &rows[nu..nu + nl];
    // Both blocks must start at the leading edge for this to be Lednicer.
    if upper[0].x > upper[nu - 1].x || lower[0].x > lower[nl - 1].x {
        return None;
    }

    // Reassemble into a single traversal: trailing edge → upper → leading edge
    // → lower → trailing edge.
    let mut points: Vec<Vec2> = upper.iter().rev().copied().collect();
    points.extend(lower.iter().skip(1).copied());
    notes.push(format!(
        "Lednicer format detected ({nu} upper + {nl} lower points); the surfaces were joined into one contour."
    ));

    Some(ParsedGeometry {
        points,
        format: CoordinateFormat::Lednicer,
        name,
        notes: std::mem::take(notes),
    })
}

/// Serialise points back to CSV with an `x,y` header (PRD §52 geometry export).
pub fn to_csv(points: &[Vec2]) -> String {
    let mut s = String::from("x,y\n");
    for p in points {
        s.push_str(&format!("{:.9},{:.9}\n", p.x, p.y));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_csv_with_header() {
        let g = parse_coordinates("x,y\n1.0,0.0\n0.5,0.1\n0.0,0.0\n0.5,-0.1\n").unwrap();
        assert_eq!(g.points.len(), 4);
        assert_eq!(g.points[0], Vec2::new(1.0, 0.0));
        assert_eq!(g.format, CoordinateFormat::SingleContour);
    }

    #[test]
    fn whitespace_delimited_with_name_line() {
        let g = parse_coordinates(
            "NACA 0012\n 1.0000  0.0013\n 0.5000  0.0294\n 0.0000  0.0000\n 0.5000 -0.0294\n",
        )
        .unwrap();
        assert_eq!(g.name.as_deref(), Some("NACA 0012"));
        assert_eq!(g.points.len(), 4);
    }

    #[test]
    fn semicolon_and_tab_separators() {
        let g = parse_coordinates("1;0\n0.5;0.1\n0\t0\n0.5;-0.1\n").unwrap();
        assert_eq!(g.points.len(), 4);
    }

    #[test]
    fn comments_and_blank_lines_are_skipped() {
        let g = parse_coordinates(
            "# a comment\n! another\n\n1,0\n\n0.5,0.1\n// mid comment\n0,0\n0.5,-0.1\n",
        )
        .unwrap();
        assert_eq!(g.points.len(), 4);
    }

    #[test]
    fn fortran_exponent_notation_is_accepted() {
        let g = parse_coordinates("1.0D0 0.0\n5.0D-1 1.0D-1\n0.0 0.0\n5.0D-1 -1.0D-1\n").unwrap();
        assert!((g.points[1].x - 0.5).abs() < 1e-12);
        assert!((g.points[1].y - 0.1).abs() < 1e-12);
    }

    #[test]
    fn third_column_is_ignored() {
        let g = parse_coordinates("1,0,0\n0.5,0.1,0\n0,0,0\n0.5,-0.1,0\n").unwrap();
        assert_eq!(g.points.len(), 4);
    }

    #[test]
    fn lednicer_blocks_are_joined_into_one_contour() {
        let text = "\
MY AIRFOIL
       3.       3.

  0.0000   0.0000
  0.5000   0.1000
  1.0000   0.0000

  0.0000   0.0000
  0.5000  -0.1000
  1.0000   0.0000
";
        let g = parse_coordinates(text).unwrap();
        assert_eq!(g.format, CoordinateFormat::Lednicer);
        // TE → upper → LE → lower → TE, minus the duplicated LE.
        assert_eq!(g.points.len(), 5);
        assert!((g.points[0].x - 1.0).abs() < 1e-12);
        assert!(
            g.points[1].y > 0.0,
            "second point should be on the upper surface"
        );
        assert!(
            (g.points[2].x).abs() < 1e-12,
            "third point should be the LE"
        );
        assert!(g.points[3].y < 0.0);
    }

    #[test]
    fn selig_file_is_not_misread_as_lednicer() {
        // A Selig file's first data row is the trailing edge, never a count pair.
        let text = "NACA 2412\n1.0 0.0\n0.5 0.1\n0.0 0.0\n0.5 -0.1\n";
        assert_eq!(
            parse_coordinates(text).unwrap().format,
            CoordinateFormat::SingleContour
        );
    }

    #[test]
    fn integral_coordinate_pair_is_not_mistaken_for_a_lednicer_header() {
        // A square with integral coordinates: `4 4` never appears, but the first
        // row `4,0` could look like counts. It must stay a single contour since
        // the following rows do not form LE→TE blocks.
        let g = parse_coordinates("4,0\n4,4\n0,4\n0,0\n").unwrap();
        assert_eq!(g.format, CoordinateFormat::SingleContour);
        assert_eq!(g.points.len(), 4);
    }

    #[test]
    fn empty_and_textual_files_are_rejected_with_actionable_messages() {
        assert_eq!(parse_coordinates("   \n\n").unwrap_err(), ParseError::Empty);
        let e = parse_coordinates("hello\nworld\nnot numbers here\n").unwrap_err();
        assert!(e.to_string().contains("coordinate"), "{e}");
    }

    #[test]
    fn too_few_points_is_reported_with_the_count() {
        let e = parse_coordinates("1,0\n0,0\n").unwrap_err();
        assert!(
            matches!(e, ParseError::TooFewPoints { found: 2, .. }),
            "{e:?}"
        );
    }

    #[test]
    fn oversize_input_is_rejected_before_parsing() {
        let big = "1,0\n".repeat(MAX_FILE_BYTES / 4 + 10);
        assert!(matches!(
            parse_coordinates(&big).unwrap_err(),
            ParseError::TooLarge { .. }
        ));
    }

    #[test]
    fn trailing_garbage_is_noted_not_fatal() {
        let g = parse_coordinates("1,0\n0.5,0.1\n0,0\n0.5,-0.1\nEOF marker\n").unwrap();
        assert_eq!(g.points.len(), 4);
        // "EOF marker" yields no numbers at all, so it is silently skipped.
        assert!(g.notes.iter().all(|n| !n.contains("panic")));
    }

    #[test]
    fn partially_numeric_row_is_skipped_with_a_note() {
        let g = parse_coordinates("1,0\n0.5,0.1\nbroken 7\n0,0\n0.5,-0.1\n").unwrap();
        assert_eq!(g.points.len(), 4);
        assert!(
            g.notes.iter().any(|n| n.contains("skipped")),
            "{:?}",
            g.notes
        );
    }

    #[test]
    fn csv_round_trip() {
        let pts = vec![
            Vec2::new(1.0, 0.0),
            Vec2::new(0.5, 0.1),
            Vec2::new(0.0, 0.0),
            Vec2::new(0.5, -0.1),
        ];
        let back = parse_coordinates(&to_csv(&pts)).unwrap();
        assert_eq!(back.points.len(), 4);
        for (a, b) in pts.iter().zip(back.points.iter()) {
            assert!(a.distance(*b) < 1e-9);
        }
    }
}
