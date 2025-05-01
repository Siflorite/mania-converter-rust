use mania_converter::malody_func::process_whole_dir_mcz;
use mania_converter::osu_func::parse_whole_dir_osz;
use std::io::{self, Write};

fn main() -> io::Result<()> {
    let mode = read_bool_input(
        "Please choose mode (y for mcz converter, n for osz info card): ",
        true // 默认值
    )?;

    if mode {
        println!("Malody MCZ to Osu! OSZ Converter");
        println!("--------------------------------");

        // 获取是否计算 SR
        let calc_sr = read_bool_input(
            "Calculate star rating for beatmaps? (y/n): ",
            true // 默认值
        )?;

        // 获取是否打印结果
        let print_results = read_bool_input(
            "Show conversion summary? (y/n): ",
            true // 默认值
        )?;

        process_whole_dir_mcz("", calc_sr, print_results)?;

        println!("\nConversion completed successfully!");
        println!("\nPress Enter to exit...");
        io::stdin().read_line(&mut String::new())?;
    } else {
        let locations = parse_whole_dir_osz("")?;
        println!("\nInfo cards generated successfully! Locations:");
        for p in locations { println!("{p}") }
        println!("\nPress Enter to exit...");
        io::stdin().read_line(&mut String::new())?;
    }

    Ok(())
}

/// 通用布尔值输入读取函数
fn read_bool_input(prompt: &str, default: bool) -> io::Result<bool> {
    let mut input = String::new();
    let mut retry_count = 0;
    const MAX_RETRIES: u8 = 3;

    loop {
        print!("{}", prompt);
        io::stdout().flush()?; // 确保立即显示提示

        input.clear();
        io::stdin().read_line(&mut input)?;

        // 解析输入
        match input.trim().to_lowercase().as_str() {
            "y" | "yes" | "t" | "true" => return Ok(true),
            "n" | "no" | "f" | "false" => return Ok(false),
            "" => {
                println!("Using default value: {}", default);
                return Ok(default);
            }
            _ => {
                retry_count += 1;
                if retry_count >= MAX_RETRIES {
                    println!("Invalid input after {} attempts. Using default: {}", MAX_RETRIES, default);
                    return Ok(default);
                }
                println!("Please enter 'y' or 'n' (or leave empty for default)");
            }
        }
    }
}