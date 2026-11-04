//! Direct Windows PC/SC. No server, browser, Python or external runtime.
use anyhow::{Result, bail};
use std::ptr::{null, null_mut};
#[repr(C)]
struct State {
    reader: *const u16,
    user: *mut std::ffi::c_void,
    current: u32,
    event: u32,
    atr_len: u32,
    atr: [u8; 36],
}
#[repr(C)]
struct Pci {
    protocol: u32,
    length: u32,
}
#[link(name = "winscard", kind = "raw-dylib")]
unsafe extern "system" {
    fn SCardEstablishContext(scope: u32, a: *const u8, b: *const u8, context: *mut usize) -> i32;
    fn SCardReleaseContext(context: usize) -> i32;
    fn SCardListReadersW(
        context: usize,
        groups: *const u16,
        readers: *mut u16,
        length: *mut u32,
    ) -> i32;
    fn SCardGetStatusChangeW(context: usize, timeout: u32, states: *mut State, count: u32) -> i32;
    fn SCardConnectW(
        context: usize,
        reader: *const u16,
        share: u32,
        protocols: u32,
        card: *mut usize,
        protocol: *mut u32,
    ) -> i32;
    fn SCardReconnect(
        card: usize,
        share: u32,
        protocols: u32,
        initialization: u32,
        protocol: *mut u32,
    ) -> i32;
    fn SCardDisconnect(card: usize, disposition: u32) -> i32;
    fn SCardTransmit(
        card: usize,
        pci: *const Pci,
        tx: *const u8,
        length: u32,
        recv_pci: *mut Pci,
        rx: *mut u8,
        rx_length: *mut u32,
    ) -> i32;
}
fn check(code: i32) -> Result<()> {
    if code != 0 {
        bail!(match code as u32 {
            0x8010000c | 0x80100069 => "Insere o cartão no leitor.",
            0x8010002e => "Liga o leitor USB.",
            0x8010000b => "O cartão está ocupado por outra aplicação.",
            0x80100017 => "O leitor foi desligado.",
            0x8010001d => "O serviço Smart Card do Windows não está disponível.",
            _ => "Não foi possível comunicar com o cartão.",
        });
    }
    Ok(())
}
pub struct Context(usize);
impl Context {
    pub fn new() -> Result<Self> {
        let mut ctx = 0;
        unsafe {
            check(SCardEstablishContext(0, null(), null(), &mut ctx))?;
        }
        Ok(Self(ctx))
    }
    pub fn readers(&self) -> Result<Vec<String>> {
        unsafe {
            let mut n = 0;
            let code = SCardListReadersW(self.0, null(), null_mut(), &mut n);
            if code as u32 == 0x8010002e {
                return Ok(vec![]);
            }
            check(code)?;
            let mut buf = vec![0; n as usize];
            check(SCardListReadersW(self.0, null(), buf.as_mut_ptr(), &mut n))?;
            Ok(buf
                .split(|c| *c == 0)
                .filter(|s| !s.is_empty())
                .map(String::from_utf16_lossy)
                .collect())
        }
    }
    pub fn present(&self, reader: &str) -> Result<bool> {
        let name: Vec<u16> = reader.encode_utf16().chain(Some(0)).collect();
        let mut state = State {
            reader: name.as_ptr(),
            user: null_mut(),
            current: 0,
            event: 0,
            atr_len: 0,
            atr: [0; 36],
        };
        unsafe {
            check(SCardGetStatusChangeW(self.0, 0, &mut state, 1))?;
        }
        Ok(state.event & 0x20 != 0 && state.event & (0x4 | 0x8 | 0x200) == 0)
    }
    pub fn connect(&self, reader: &str) -> Result<Card<'_>> {
        let name: Vec<u16> = reader.encode_utf16().chain(Some(0)).collect();
        let mut card = Card {
            handle: 0,
            protocol: 0,
            _context: self,
        };
        unsafe {
            check(SCardConnectW(
                self.0,
                name.as_ptr(),
                1,
                3,
                &mut card.handle,
                &mut card.protocol,
            ))?;
            check(SCardReconnect(card.handle, 1, 3, 1, &mut card.protocol))?;
        }
        Ok(card)
    }
}
impl Drop for Context {
    fn drop(&mut self) {
        unsafe {
            SCardReleaseContext(self.0);
        }
    }
}
pub struct Card<'a> {
    handle: usize,
    protocol: u32,
    _context: &'a Context,
}
impl Drop for Card<'_> {
    fn drop(&mut self) {
        if self.handle != 0 {
            unsafe {
                SCardDisconnect(self.handle, 1);
            }
        }
    }
}
impl Card<'_> {
    fn send(&self, data: &[u8]) -> Result<Vec<u8>> {
        let pci = Pci {
            protocol: self.protocol,
            length: 8,
        };
        let mut rx = vec![0; 258];
        let mut n = 258;
        unsafe {
            check(SCardTransmit(
                self.handle,
                &pci,
                data.as_ptr(),
                data.len() as u32,
                null_mut(),
                rx.as_mut_ptr(),
                &mut n,
            ))?;
        }
        rx.truncate(n as usize);
        if rx.len() < 2 {
            bail!("Resposta incompleta do cartão.")
        }
        Ok(rx)
    }
    pub fn transmit(&mut self, input: &[u8]) -> Result<Vec<u8>> {
        transmit(input, |c| self.send(c))
    }
}
pub fn transmit(input: &[u8], mut send: impl FnMut(&[u8]) -> Result<Vec<u8>>) -> Result<Vec<u8>> {
    let mut command = input.to_vec();
    let case4 = command.len() >= 5
        && (command.len() == 6 + command[4] as usize
            || matches!(command[1], 0xa4 | 0x8a | 0x8e)
                && command.len() == 5 + command[4] as usize);
    if case4 && command.len() == 5 + command[4] as usize {
        command.push(0)
    }
    let mut response = send(&command)?;
    if response[response.len() - 2] == 0x6c && command.len() >= 5 {
        let le = *response.last().unwrap();
        let corrected = if case4 {
            vec![command[0], 0xc0, 0, 0, le]
        } else {
            let mut c = command.clone();
            *c.last_mut().unwrap() = le;
            c
        };
        response = send(&corrected)?;
    }
    let mut payload = vec![];
    for _ in 0..8 {
        if response[response.len() - 2] != 0x61 {
            payload.extend(response);
            return Ok(payload);
        }
        payload.extend(&response[..response.len() - 2]);
        let mut follow = vec![command[0], 0xc0, 0, 0, *response.last().unwrap()];
        response = send(&follow)?;
        if response[response.len() - 2] == 0x6c {
            follow[4] = *response.last().unwrap();
            response = send(&follow)?;
        }
    }
    bail!("Limite de respostas do cartão excedido.")
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn open_session_not_replayed() {
        let command = [0x94, 0x8a, 0x89, 0x38, 4, 1, 2, 3, 4];
        let mut sent = vec![];
        let mut responses = vec![vec![0x6c, 0x22], vec![0x90, 0]].into_iter();
        transmit(&command, |c| {
            sent.push(c.to_vec());
            Ok(responses.next().unwrap())
        })
        .unwrap();
        assert_eq!(sent[0], [&command[..], &[0][..]].concat());
        assert_eq!(sent[1], [0x94, 0xc0, 0, 0, 0x22]);
    }
}
