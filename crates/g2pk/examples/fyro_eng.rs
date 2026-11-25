fn main() -> g2pk::Result<()> {
    let g2p = g2pk::G2p::new()?;
    for w in ["mileage", "bliss", "point", "card", "cash", "skypass", "file", "lounge", "amex"] {
        println!("  {:<9} -> {}", w, g2p.convert(w)?);
    }
    Ok(())
}
