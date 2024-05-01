#![warn(rust_2018_idioms)]
#![warn(clippy::pedantic)]

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

use anyhow::{anyhow, Context};
use chrono::{serde::ts_nanoseconds, DateTime, Local, Utc};
use clap::{crate_name, crate_version, value_parser, Arg, Command};
use nom::{
    bytes::complete::tag,
    character::complete::{newline, u8},
    combinator::all_consuming,
    IResult,
};

#[derive(Debug, serde::Deserialize)]
struct Record {
    start: u64,
    end: u64,
    #[serde(with = "ts_nanoseconds")]
    timestamp: DateTime<Utc>,
    name: String,
    _hash: String,
}

fn parse_header_help(input: &[u8]) -> IResult<&[u8], u8> {
    let (i, _) = tag(b"# ninja log v")(input)?;
    let (i, d) = u8(i)?;
    let (i, _) = newline(i)?;

    Ok((i, d))
}

fn parse_header(input: &[u8]) -> IResult<&[u8], u8> {
    all_consuming(parse_header_help)(input)
}

fn main() -> Result<(), anyhow::Error> {
    let matches = Command::new(crate_name!())
        .version(crate_version!())
        .arg(
            Arg::new(".ninja_log")
                .index(1)
                .required(true)
                .value_parser(value_parser!(PathBuf)),
        )
        .get_matches();

    let filename = matches
        .get_one::<PathBuf>(".ninja_log")
        .expect("file path to .ninja_log file");

    let file = File::open(filename).with_context(|| format!("opening {}", filename.display()))?;
    let mut buf_reader = BufReader::new(file);

    // read version header
    let mut header_line = Vec::new();
    buf_reader
        .read_until(b'\n', &mut header_line)
        .context("try to read ninja log header line")?;
    match parse_header(&header_line) {
        Ok((_, version)) => {
            if version != 5 {
                return Err(anyhow!("unsupported Ninja log version {version}"));
            }
        }
        Err(_) => return Err(anyhow!("cannot parse header line")),
    }

    // read the actual CSV data
    let reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .comment(Some(b'#'))
        .delimiter(b'\t')
        .from_reader(buf_reader);

    for result in reader.into_deserialize() {
        let record: Record = result.context("reading record")?;
        let duration = record.end - record.start;
        println!(
            "{duration} {} {}",
            record.name,
            DateTime::<Local>::from(record.timestamp)
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use assert_matches::assert_matches;

    #[test]
    fn test_parse_header() {
        assert_eq!(super::parse_header(b"# ninja log v5\n"), Ok((&[][..], 5u8)));
        assert_eq!(super::parse_header(b"# ninja log v4\n"), Ok((&[][..], 4u8)));

        assert_matches!(super::parse_header(b"# ninja log v4"), Err(_));
    }
}
