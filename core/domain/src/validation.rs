use crate::DomainError;

pub(crate) fn validate_nonempty(value: &str) -> Result<(), DomainError> {
    if value.is_empty() {
        Err(DomainError::EmptyValue)
    } else {
        Ok(())
    }
}
fn lower_hex(b: u8) -> bool {
    b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
}
pub(crate) fn validate_uuid_v7(value: &str) -> Result<(), DomainError> {
    let b = value.as_bytes();
    let valid = b.len() == 36
        && b.iter().enumerate().all(|(i, &c)| {
            if [8, 13, 18, 23].contains(&i) {
                c == b'-'
            } else {
                lower_hex(c)
            }
        })
        && b[14] == b'7'
        && matches!(b[19], b'8' | b'9' | b'a' | b'b');
    if valid {
        Ok(())
    } else {
        Err(DomainError::InvalidUuidV7)
    }
}
pub(crate) fn validate_action_type(value: &str) -> Result<(), DomainError> {
    let valid = value.contains('.')
        && value.split('.').all(|part| {
            part.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
                && part
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        });
    if valid {
        Ok(())
    } else {
        Err(DomainError::InvalidActionType)
    }
}
pub(crate) fn validate_sha256(value: &str) -> Result<(), DomainError> {
    if value.len() == 64 && value.bytes().all(lower_hex) {
        Ok(())
    } else {
        Err(DomainError::InvalidSha256Hex)
    }
}
pub(crate) fn validate_currency(value: &str) -> Result<(), DomainError> {
    if value.len() == 3 && value.bytes().all(|b| b.is_ascii_uppercase()) {
        Ok(())
    } else {
        Err(DomainError::InvalidCurrencyCode)
    }
}

/// Accept canonical UTC milliseconds only. Offset normalization belongs at the boundary.
pub(crate) fn validate_timestamp(value: &str) -> Result<(), DomainError> {
    let b = value.as_bytes();
    let separators = [
        (4, b'-'),
        (7, b'-'),
        (10, b'T'),
        (13, b':'),
        (16, b':'),
        (19, b'.'),
        (23, b'Z'),
    ];
    if b.len() != 24
        || !b.iter().enumerate().all(
            |(i, c)| match separators.iter().find(|(pos, _)| *pos == i) {
                Some((_, expected)) => c == expected,
                None => c.is_ascii_digit(),
            },
        )
    {
        return Err(DomainError::InvalidTimestamp);
    }
    let number = |start: usize, end: usize| -> u32 {
        b[start..end]
            .iter()
            .fold(0, |n, c| n * 10 + u32::from(c - b'0'))
    };
    let year = number(0, 4);
    let month = number(5, 7);
    let day = number(8, 10);
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => 0,
    };
    if day == 0 || day > days || number(11, 13) > 23 || number(14, 16) > 59 || number(17, 19) > 59 {
        return Err(DomainError::InvalidTimestamp);
    }
    Ok(())
}
