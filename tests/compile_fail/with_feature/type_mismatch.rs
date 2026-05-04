use scnr2::scanner;

scanner! {
    BadScanner {
        state marker: str(r"[A-Z]+");
        mode INITIAL {
            token capture("#", marker) => 1;
        }
    }
}

fn main() {}
