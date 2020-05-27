
fn main() {
   println!("Hello world");
   let mut var: usize = 100;
   println!("{}", mutate(&mut var));
}

fn mutate(variable: &mut usize) -> usize {
    return *(variable) + 1;
}