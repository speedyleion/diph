//! Prints, as Markdown, the licenses of the bundled syntax definitions that
//! require acknowledgement when diph is distributed.

fn main() {
    println!("## Syntax definitions\n");
    println!(
        "diph includes syntax definitions curated by the\n\
         [bat project](https://github.com/sharkdp/bat), used under these licenses.\n"
    );
    for license in two_face::acknowledgement::listing().for_syntaxes() {
        println!(
            "### {}\n\n````text\n{}\n````\n",
            license.rel_path.display(),
            license.text.trim_end()
        );
    }
}
