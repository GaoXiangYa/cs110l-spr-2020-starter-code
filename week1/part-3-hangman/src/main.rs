// Simple Hangman Program
// User gets five incorrect guesses
// Word chosen randomly from words.txt
// Inspiration from: https://doc.rust-lang.org/book/ch02-00-guessing-game-tutorial.html
// This assignment will introduce you to some fundamental syntax in Rust:
// - variable declaration
// - string manipulation
// - conditional statements
// - loops
// - vectors
// - files
// - user input
// We've tried to limit/hide Rust's quirks since we'll discuss those details
// more in depth in the coming lectures.
extern crate rand;
use rand::Rng;
use std::char;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::io::Write;

const NUM_INCORRECT_GUESSES: u32 = 5;
const WORDS_PATH: &str = "words.txt";

fn pick_a_random_word() -> String {
    let file_string = fs::read_to_string(WORDS_PATH).expect("Unable to read file.");
    let words: Vec<&str> = file_string.split('\n').collect();
    String::from(words[rand::thread_rng().gen_range(0, words.len())].trim())
}

fn find_next_word_pos(word_vec: &Vec<char>, target: &char, start: usize) -> Option<usize> {
    word_vec
        .iter()
        .enumerate()
        .skip(start)
        .find(|(_, &x)| x == *target)
        .map(|(pos, _)| pos)
}

fn replace_char(s: &mut String, target: &char, pos: usize) {
    let mut vec_chars: Vec<char> = s.chars().collect();
    if pos < s.len() {
        vec_chars[pos] = *target;
        *s = vec_chars.into_iter().collect();
    }
}

fn main() {
    let secret_word = pick_a_random_word();
    // Note: given what you know about Rust so far, it's easier to pull characters out of a
    // vector than it is to pull them out of a string. You can get the ith character of
    // secret_word by doing secret_word_chars[i].
    let secret_word_chars: Vec<char> = secret_word.chars().collect();
    // Uncomment for debugging:
    // println!("random word: {}", secret_word);

    // Your code here! :)
    let secret_word_len = secret_word.len();
    let mut count = NUM_INCORRECT_GUESSES;
    let mut guessed_word: String = std::iter::repeat("-").take(secret_word_len).collect();
    let mut guessed_word_count = 0;
    let mut guessed_word_pos: HashMap<String, usize> = HashMap::new();
    let mut have_guessed_word = String::new();
    let mut guessed_word_set :HashSet<usize> = HashSet::new();

    println!("Welcome to CS110L Hangman!");

    loop {
        println!("The word so far is {}", guessed_word);
        println!(
            "You have guessed the following letters: {}",
            have_guessed_word
        );
        println!("You have {} guesses left", count);
        print!("Please guess a letter: ");
        std::io::stdout().flush().unwrap(); // 确保 prompt 立即输出

        let mut guess_word = String::new();

        std::io::stdin()
            .read_line(&mut guess_word)
            .expect("Error reading line.");

        let word = guess_word.chars().next().unwrap();
        have_guessed_word.push(word);

        let pos = guessed_word_pos.entry(guess_word).or_insert(0);
        let new_pos = find_next_word_pos(&secret_word_chars, &word, *pos);
        if new_pos.is_none() || guessed_word_set.contains(&new_pos.unwrap()) {
            count -= 1;
            println!("Sorry, that letter is not in the word");
        } else {
            guessed_word_count += 1;
            replace_char(&mut guessed_word, &word, new_pos.unwrap());
            guessed_word_set.insert(new_pos.unwrap());
        }

        if guessed_word_count == secret_word_len {
            println!(
                "Congratulations you guessed the secret word: {}",
                secret_word
            );
            break;
        } else if count == 0 {
            println!("Sorry, you ran out of guesses!");
            break;
        } else {
            continue;
        }
    }
}
