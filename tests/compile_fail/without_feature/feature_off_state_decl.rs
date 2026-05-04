use scnr2::scanner;

scanner! {
    DisabledScanner {
        state n: count(0..=4);
        mode INITIAL {
            token r"." => 99;
        }
    }
}

fn main() {}
