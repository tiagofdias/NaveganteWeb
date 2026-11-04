use crate::api::Api;
use anyhow::{Result, bail, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
pub fn id(value: &Value) -> Result<String> {
    let s = if let Some(s) = value.as_str() {
        s.to_owned()
    } else if value.is_number() {
        value.to_string()
    } else {
        bail!("Identificador em falta.")
    };
    ensure!(
        !s.is_empty()
            && s.len() <= 100
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-'),
        "Identificador inválido."
    );
    Ok(s)
}
fn read_allowed(c: &[u8]) -> bool {
    if !(4..=261).contains(&c.len()) || !matches!(c[0], 0 | 0x94) {
        return false;
    }
    match c[1] {
        0xa4 => c.len() >= 5 && (c.len() == 5 + c[4] as usize || c.len() == 6 + c[4] as usize),
        0x8a | 0x8e => {
            c.len() >= 5
                && matches!(c[4], 4 | 8)
                && (c.len() == 5 + c[4] as usize || c.len() == 6 + c[4] as usize)
        }
        0x20 => c.len() == 9 && c[2..5] == [0, 0, 4],
        0xb0 | 0xb2 | 0xc0 | 0xca => c.len() == 5,
        _ => false,
    }
}
fn write_allowed(c: &[u8]) -> bool {
    if !(5..=261).contains(&c.len()) || !matches!(c[0], 0 | 0x94) {
        return false;
    }
    match c[1] {
        0x84 | 0x7c => c.len() == 5,
        0xdc | 0xd2 | 0xe2 | 0xd6 | 0x32 | 0x30 | 0x3a | 0x38 | 0xb8 => {
            c[4] > 0 && (c.len() == 5 + c[4] as usize || c.len() == 6 + c[4] as usize)
        }
        _ => false,
    }
}
pub fn batch(
    encoded: &str,
    loading: bool,
    mut transmit: impl FnMut(&[u8]) -> Result<Vec<u8>>,
) -> Result<Value> {
    let raw = STANDARD.decode(encoded)?;
    ensure!(
        raw.len() >= 2 && raw.len() <= 65536 && raw[0] == 2,
        "Comandos de versão inválida."
    );
    let mut parsed = vec![];
    let mut offset = 2;
    while offset < raw.len() {
        ensure!(offset + 2 <= raw.len(), "Comando incompleto.");
        let opcode = raw[offset];
        let (end, apdu) = match opcode {
            73 => {
                ensure!(offset + 6 <= raw.len(), "Comando incompleto.");
                let n = u32::from_be_bytes(raw[offset + 2..offset + 6].try_into()?) as usize;
                let end = offset + 6 + n;
                ensure!(end <= raw.len(), "Comando incompleto.");
                let c = &raw[offset + 6..end];
                ensure!(
                    read_allowed(c) || (loading && write_allowed(c)),
                    "Comando não autorizado para esta operação."
                );
                (end, Some(c))
            }
            121 => (offset + 6, None),
            122 => (offset + 2, None),
            _ => bail!("Ação de cartão não suportada."),
        };
        ensure!(end <= raw.len(), "Comando incompleto.");
        parsed.push((opcode, apdu));
        offset = end;
    }
    ensure!(
        parsed.len() == raw[1] as usize,
        "Contagem de comandos inválida."
    );
    let mut answers = vec![2, raw[1]];
    for (opcode, apdu) in parsed {
        answers.extend([opcode, 0, 0, 0, 0]);
        if let Some(c) = apdu {
            let response = transmit(c)?;
            ensure!(response.len() >= 2, "Resposta incompleta.");
            answers.extend((response.len() as u32).to_be_bytes());
            answers.extend(response);
        } else if opcode == 121 {
            answers.extend([0, 0, 0, 8, 0]);
        }
    }
    Ok(json!(answers))
}
pub fn read(
    api: &Api,
    mut transmit: impl FnMut(&[u8]) -> Result<Vec<u8>>,
) -> Result<(String, Value)> {
    let mut result = api.gateway(
        "POST",
        "reads?device=tiptop",
        Some(json!([2, 1, 121, 0, 0, 0, 0, 0, 0, 0, 8, 0])),
    )?;
    let mut read_id = None;
    let (mut opened, mut pin_used) = (false, false);
    for _ in 0..48 {
        ensure!(result.is_object(), "Formato de leitura inesperado.");
        if !result["id"].is_null() {
            let next = id(&result["id"])?;
            ensure!(
                read_id.as_ref().is_none_or(|old| old == &next),
                "A leitura mudou durante a operação."
            );
            read_id = Some(next);
        }
        let rid = read_id
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Leitura sem identificador."))?;
        let Some(commands) = result["PO_command"].as_str().filter(|s| !s.is_empty()) else {
            ensure!(result["data"].is_object(), "Leitura sem dados do cartão.");
            return Ok((rid.clone(), result["data"].clone()));
        };
        let answer = batch(commands, false, |c| {
            if c[1] == 0x20 {
                ensure!(
                    opened && !pin_used,
                    "Verificação fora da sessão de leitura."
                );
                pin_used = true;
            }
            let r = transmit(c)?;
            ensure!(r.len() >= 2, "Resposta incompleta.");
            if c[1] == 0x8a {
                opened = r.ends_with(&[0x90, 0]);
                ensure!(opened, "Não foi possível autenticar a leitura.");
            }
            if c[1] == 0x20 {
                ensure!(
                    r.ends_with(&[0x90, 0]),
                    "O cartão recusou a verificação. Não foi repetida."
                );
            }
            Ok(r)
        })?;
        result = api.gateway("PUT", &format!("reads/{rid}"), Some(answer))?;
    }
    bail!("A leitura excedeu o número de passos esperado.")
}
pub fn load(
    api: &Api,
    signed: &Value,
    mut transmit: impl FnMut(&[u8]) -> Result<Vec<u8>>,
    write_started: &mut bool,
) -> Result<()> {
    for k in ["request64", "signature", "keyRef"] {
        ensure!(
            signed[k].as_str().is_some_and(|s| !s.is_empty()),
            "Autorização de carregamento em falta."
        );
    }
    let original: Value =
        serde_json::from_slice(&STANDARD.decode(signed["request64"].as_str().unwrap())?)?;
    let path = original["path"].as_str().unwrap_or("");
    ensure!(
        original["method"] == "POST"
            && regex::Regex::new(r"^loads(?:\?[A-Za-z0-9_=&%.-]*)?$")?.is_match(path),
        "Autorização de carregamento inválida."
    );
    let mut result=api.exchange(json!({"id":201,"version":1,"request64":signed["request64"],"signature":signed["signature"],"keyRef":signed["keyRef"]}))?;
    let mut load_id = None;
    let mut selected = false;
    for _ in 0..64 {
        ensure!(result.is_object(), "Resposta de carregamento inválida.");
        if !result["id"].is_null() {
            let next = id(&result["id"])?;
            ensure!(
                load_id.as_ref().is_none_or(|old| old == &next),
                "O carregamento mudou durante a operação."
            );
            load_id = Some(next);
        }
        let Some(commands) = result["PO_command"].as_str().filter(|s| !s.is_empty()) else {
            return Ok(());
        };
        let lid = load_id
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Carregamento sem identificador."))?;
        let answers = batch(commands, true, |c| {
            if !selected && c[1] != 0xa4 {
                let r = transmit(&[
                    0, 0xa4, 4, 0, 8, 0x31, 0x54, 0x49, 0x43, 0x2e, 0x49, 0x43, 0x41,
                ])?;
                ensure!(
                    r.ends_with(&[0x90, 0]),
                    "Não foi possível selecionar o passe."
                );
            }
            selected = true;
            if !matches!(
                c[1],
                0xa4 | 0xb0 | 0xb2 | 0xc0 | 0xca | 0x8a | 0x20 | 0x84 | 0x7c
            ) {
                *write_started = true;
            }
            transmit(c)
        })?;
        result = api.gateway("PUT", &format!("loads/{lid}"), Some(answers))?;
    }
    bail!("O carregamento excedeu o número de passos esperado.")
}
#[cfg(test)]
mod tests {
    use super::*;
    fn encode(commands: &[&[u8]]) -> String {
        let mut raw = vec![2, commands.len() as u8];
        for c in commands {
            raw.extend([73, 1]);
            raw.extend((c.len() as u32).to_be_bytes());
            raw.extend(*c);
        }
        STANDARD.encode(raw)
    }
    #[test]
    fn validates_entire_batch_before_sending() {
        let mut called = false;
        assert!(
            batch(
                &encode(&[&[0x94, 0xb2, 1, 0x44, 0x1d], &[0x94, 0xdc, 1, 0x44, 1, 0]]),
                false,
                |_| {
                    called = true;
                    Ok(vec![0x90, 0])
                }
            )
            .is_err()
        );
        assert!(!called);
    }
    #[test]
    fn response_encoding() {
        assert_eq!(
            batch(&encode(&[&[0x94, 0xb2, 1, 0x44, 0x1d]]), false, |_| Ok(
                vec![0x12, 0x90, 0]
            ))
            .unwrap(),
            json!([2, 1, 73, 0, 0, 0, 0, 0, 0, 0, 3, 18, 144, 0])
        );
    }
    #[test]
    fn key_changes_never_allowed() {
        for ins in [0xd8, 4, 0x44] {
            assert!(
                batch(
                    &encode(&[&[0x94, ins, 0, 0, 4, 0, 0, 0, 0]]),
                    true,
                    |_| panic!("Must not transmit")
                )
                .is_err()
            );
        }
    }
    #[test]
    fn malformed_batches() {
        for raw in [vec![2, 1, 73, 1], vec![2, 2], vec![1, 0]] {
            assert!(
                batch(&STANDARD.encode(raw), false, |_| panic!(
                    "Must not transmit"
                ))
                .is_err()
            );
        }
    }
    #[test]
    fn pin_requires_open_authenticated_session() {
        let (api, worker) = crate::test_support::api(vec![
            json!({"status":200,"body":{"id":"test-read","PO_command":encode(&[&[0x94,0x20,0,0,4,1,2,3,4]])}}),
        ]);
        assert!(read(&api, |_| panic!("Must not transmit")).is_err());
        worker.join().unwrap();
    }
    #[test]
    fn signed_load_stays_on_load_resource() {
        let signed = json!({"request64":STANDARD.encode(br#"{"method":"POST","path":"loads"}"#),"signature":"server-signature","keyRef":"server-key"});
        let (api, worker) = crate::test_support::api(vec![
            json!({"status":200,"body":{"id":"test-load","PO_command":encode(&[&[0x94,0xdc,1,0x44,1,0]])}}),
            json!({"status":200,"body":{"id":"test-load","progress":100}}),
        ]);
        let mut sent = vec![];
        let mut started = false;
        load(
            &api,
            &signed,
            |c| {
                sent.push(c.to_vec());
                Ok(vec![0x90, 0])
            },
            &mut started,
        )
        .unwrap();
        assert!(started);
        assert_eq!(sent.len(), 2);
        assert_eq!(sent[0][1], 0xa4);
        let requests = worker.join().unwrap();
        assert!(requests[0].contains("request64"));
        assert!(requests[1].contains("loads/test-load"));
    }
    #[test]
    fn signed_load_rejects_arbitrary_path() {
        let signed = json!({"request64":STANDARD.encode(br#"{"method":"POST","path":"other"}"#),"signature":"server-signature","keyRef":"server-key"});
        let api = Api::mock("http://127.0.0.1:1/".into());
        assert!(load(&api, &signed, |_| panic!("Must not transmit"), &mut false).is_err());
    }
}
