use std::time::Duration;

fn main() {
    let greeting = "Hello, world!";
    println!("{}", greeting);

    let numbers = vec![1, 2, 3, 4, 5];
    let sum: i32 = numbers.iter().sum();
    println!("Sum: {}", sum);

    let option_value: Option<String> = Some("test".to_string());
    if let Some(value) = option_value {
        println!("Value: {}", value);
    }

    let result: Result<i32, String> = Ok(42);
    match result {
        Ok(n) => println!("Number: {}", n),
        Err(e) => eprintln!("Error: {}", e),
    }
}
