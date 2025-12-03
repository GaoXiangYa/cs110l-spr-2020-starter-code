use std::env;
use std::error::Error;
use std::fs::File;
use std::io;
use std::io::BufRead;
use std::process;

fn read_files(filename: &String, line_count: &mut usize) -> Result<Vec<String>, io::Error> {
    let file = File::open(filename)?;
    let mut file_vec: Vec<String> = vec![];
    for line in io::BufReader::new(file).lines() {
        let content = line?;
        *line_count += 1;
        file_vec.push(content);
    }

    Ok(file_vec)
}

fn count_words_characters(file_vec: &Vec<String>) -> (usize, usize) {
    let mut word_count: usize = 0;
    let mut character_count: usize = 0;

    let get_character_count = |content: &String| -> usize {
        content.bytes().filter(|&x| !x.is_ascii_whitespace()).count()
    };

    let get_word_count = |content: &String| -> usize {
        content.split_ascii_whitespace().count()
    };

    for content in file_vec {
        character_count += get_character_count(&content);
        word_count += get_word_count(&content);
    }

    (word_count, character_count)
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("Too few arguments.");
        process::exit(1);
    }
    let filename = &args[1];
    // Your code here :)
    let mut line_count: usize = 0;
    let file_vec = read_files(filename, &mut line_count)?;

    let (word_count, character_count) = count_words_characters(&file_vec);

    println!("word count = {}", word_count);
    println!("character count = {}", character_count);
    println!("line count = {}", line_count);

    Ok(())
}
