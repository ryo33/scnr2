use scnr2::scanner;

scanner! {
    DisabledScanner {
        mode INITIAL {
            token capture("#", n) => 1;
        }
    }
}

fn main() {}
