pub mod nouns;

#[tokio::main]
async fn main() -> clap_noun_verb::Result<()> {
    clap_noun_verb::run()
}
