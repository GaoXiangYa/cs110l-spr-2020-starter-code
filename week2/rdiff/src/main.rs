use grid::Grid;
use std::cmp::max;
// For lcs()
use std::env;
use std::error::Error;
use std::fs::File; // For read_file_lines()
use std::io::{self, BufRead}; // For read_file_lines()
use std::process;

pub mod grid;

/// Reads the file at the supplied path, and returns a vector of strings.
fn read_file_lines(filename: &String) -> Result<Vec<String>, io::Error> {
    // Be sure to delete the #[allow(unused)] line above
    let file = File::open(filename)?;
    let mut file_vec: Vec<String> = vec![];
    for line in io::BufReader::new(file).lines() {
        let content = line?;
        file_vec.push(content);
    }
    Ok(file_vec)
}

fn lcs(seq1: &Vec<String>, seq2: &Vec<String>) -> Grid {
    // Note: Feel free to use unwrap() in this code, as long as you're basically certain it'll
    // never happen. Conceptually, unwrap() is justified here, because there's not really any error
    // condition you're watching out for (i.e. as long as your code is written correctly, nothing
    // external can go wrong that we would want to handle in higher-level functions). The unwrap()
    // calls act like having asserts in C code, i.e. as guards against programming error.
    // Be sure to delete the #[allow(unused)] line above
    let seq1_len = seq1.len();
    let seq2_len = seq2.len();
    let mut grid: Grid = Grid::new(seq1_len + 1, seq2_len + 1);
    for i in 0..seq1_len + 1 {
        let _ = grid.set(i, 0, 0);
    }
    for j in 0..seq2_len + 1 {
        let _ = grid.set(0, j, 0);
    }

    for i in 0..seq1_len {
        for j in 0..seq2_len {
            if seq1[i] == seq2[j] {
                let val = grid.get(i, j).unwrap();
                let _ = grid.set(i + 1, j + 1, val + 1);
            } else {
                let val = max(grid.get(i + 1, j).unwrap(), grid.get(i, j + 1).unwrap());
                let _ = grid.set(i + 1, j + 1, val);
            }
        }
    }
    grid
}

fn print_diff(lcs_table: &Grid, lines1: &Vec<String>, lines2: &Vec<String>, i: usize, j: usize) {
    // Be sure to delete the #[allow(unused)] line above
    if i > 0 && j > 0 && lines1[i - 1] == lines2[j - 1] {
        print_diff(lcs_table, lines1, lines2, i - 1, j - 1);
        println!(" {}", lines1[i - 1]);
    } else if j > 0 && (i == 0 || lcs_table.get(i, j - 1) >= lcs_table.get(i - 1, j)) {
        print_diff(lcs_table, lines1, lines2, i, j - 1);
        println!("> {}", lines2[j - 1]);
    } else if i > 0 && (j == 0 || lcs_table.get(i, j - 1) < lcs_table.get(i - 1, j)) {
        print_diff(lcs_table, lines1, lines2, i - 1, j);
        println!("< {}", lines1[i - 1]);
    } else {
        println!("");
    }
}

#[allow(unused)] // TODO: delete this line when you implement this function
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        println!("Too few arguments.");
        process::exit(1);
    }
    let filename1 = &args[1];
    let filename2 = &args[2];

    // Be sure to delete the #[allow(unused)] line above
    let seq1 = read_file_lines(filename1)?;
    let seq2 = read_file_lines(filename2)?;
    let grid = lcs(&seq1, &seq2);

    let seq1_len = seq1.len();
    let seq2_len = seq2.len();
    print_diff(&grid, &seq1, &seq2, seq1_len, seq2_len);
    

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_read_file_lines() {
        let lines_result = read_file_lines(&String::from("handout-a.txt"));
        assert!(lines_result.is_ok());
        let lines = lines_result.unwrap();
        assert_eq!(lines.len(), 8);
        assert_eq!(
            lines[0],
            "This week's exercises will continue easing you into Rust and will feature some"
        );
    }

    #[test]
    fn test_lcs() {
        let mut expected = Grid::new(5, 4);
        expected.set(1, 1, 1).unwrap();
        expected.set(1, 2, 1).unwrap();
        expected.set(1, 3, 1).unwrap();
        expected.set(2, 1, 1).unwrap();
        expected.set(2, 2, 1).unwrap();
        expected.set(2, 3, 2).unwrap();
        expected.set(3, 1, 1).unwrap();
        expected.set(3, 2, 1).unwrap();
        expected.set(3, 3, 2).unwrap();
        expected.set(4, 1, 1).unwrap();
        expected.set(4, 2, 2).unwrap();
        expected.set(4, 3, 2).unwrap();

        println!("Expected:");
        expected.display();
        let result = lcs(
            &"abcd".chars().map(|c| c.to_string()).collect(),
            &"adb".chars().map(|c| c.to_string()).collect(),
        );
        println!("Got:");
        result.display();
        assert_eq!(result.size(), expected.size());
        for row in 0..expected.size().0 {
            for col in 0..expected.size().1 {
                assert_eq!(result.get(row, col), expected.get(row, col));
            }
        }
    }
}
