use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub enum Resp{
    Num(i64),
    SimpleStr(String),
    BulkStr(String),
    Nil,
    Arr(VecDeque<Resp>),
}


impl Resp{
    pub fn get_str(self) -> Option<String>{
        match self{
            Resp::BulkStr(s) => Some(s),
            Resp::SimpleStr(s) => Some(s),
            _ => None
        }
    }

    pub fn if_str(&self) -> bool{
        match self{
            Resp::BulkStr(_) | Resp::SimpleStr(_) => true,
            _ => false
        }
    }

    pub fn get_int(self) -> Option<i64>{
        match self{
            Resp::Num(n) => Some(n),
            _ => None
        }
    }
}

//pub type InputResult<F> = Result<F, InputError>;
pub type InputError = String;

const CLRF: [u8;2] = [13, 10];
const STR_CLRF : &'static str = "\r\n"; 
//const NILL_STR : &'static str = "$-1\r\n";

fn check_clrf(input : &[u8]) -> Result<&[u8], InputError>{
    if input.starts_with(&CLRF){
        return Ok(&input[2..])
    }
    return Err("clrf not found".to_owned())
}

fn decode_string(input : &[u8]) -> Option<(String, &[u8])>{
    let (len, strt) = input.split_at(1);
    let str_len = std::str::from_utf8(len)
            .ok()
            .and_then(|x| x.parse::<i64>().ok())?;

    check_clrf(strt)
        .ok()
        .and_then(|inp|{
            std::str::from_utf8(&inp[..(str_len as usize)])
                .map(|y| (y.to_string(), &inp[(str_len as usize)..]))
                .ok()
        })
        .and_then(|(res_str, rest)|
            check_clrf(rest)
            .map(|reste| (res_str, reste))
            .ok()
        )
}

fn decode_simple_string(input : &[u8]) -> Option<(String, &[u8])>{
    let mut iter = 0;
    while input[iter] != b'\\'{
        iter += 1;
    }
    let res_str = std::str::from_utf8(&input[..(iter+1)])
        .map(String::from)
        .ok()?;
    
    let rest = check_clrf(&input[(iter+1)..]).ok()?;
    Some((res_str, rest))
}

fn decode_int(input: &[u8]) -> Option<(i64, &[u8])>{
    let mut n = 0;
    let mut pointer = 0;
    while pointer < input.len() && input[pointer].is_ascii_digit(){
        let digit = std::str::from_utf8(&input[pointer..(pointer +1)]).ok()
            .and_then(|x| x.parse::<i64>().ok())
            .unwrap_or(0);
        n = n *10 + digit;
        pointer += 1;
    }
    check_clrf(&input[pointer..])
        .map(|rest| (n, rest))
        .ok()
}
// *1\r\n$4\r\nping\r\n
fn decode_list(input : &[u8]) -> Option<(VecDeque<Resp>, &[u8])>{
    let (length, mut reste) = decode_int(input)?;
    let mut vec_res = VecDeque::with_capacity(length as usize);
    if length == 0{
        return Some((vec_res, reste))
    }
    let mut iter = 0;
    while iter < length  {
        if let Some((resp_result, rester)) = decode_resp(reste){
            
            vec_res.push_back(resp_result);
            reste = rester;
            iter += 1;
        }
        else{ break; }
    }

    return Some((vec_res, reste))
}


// pub trait Decoder {
//     type Output;
//     fn decode(input : &[u8]) -> Option<Self::Output>;
// }

// impl Decoder for Resp{
//     type Output = (Self, &'static [u8]);

pub fn decode_resp(input : &[u8]) -> Option<(Resp, &[u8])>{
    match input.split_at(1){
        (b"*", rest) => decode_list(rest).map(|(res, rest)| (Resp::Arr(res), rest)),
        (b"$", rest) => decode_string(rest).map(|(res, rest)| (Resp::BulkStr(res), rest)),
        (b":", rest) => decode_int(rest).map(|(res, rest)| (Resp::Num(res), rest)),
        (b"+", rest) => decode_simple_string(rest).map(|(res, rest)| (Resp::SimpleStr(res),rest)),
        (_head, _tail) => {
            None
        },
    }
}


pub trait Encoder {
    type Output;
    fn encode(self) -> Option<Self::Output>;
}

impl Encoder for Resp{
    type Output = String;
    fn encode(self) -> Option<Self::Output>{
        match self{
            Resp::BulkStr(s) =>
                Some(["$", format!("{}", s.len()).as_str(), STR_CLRF, s.as_str(), STR_CLRF].concat().to_owned()),
            Resp::SimpleStr(s) =>
                Some(["+", s.as_str(), STR_CLRF].concat().to_owned()),
            Resp::Nil =>
                Some("$-1\r\n".to_string()),
            _ => None,
        }
    }
}

#[cfg(test)]
pub mod tests{
    use super::*;
    #[test]
    pub fn test_1(){
        let input = "*1\r\n$4\r\nping\r\n".as_bytes();
        let (_res, _ ) = decode_resp(input).unwrap();
       // println!("{:?}", res);
    }

    #[test]
    pub fn test_clrf(){
        let _input = "\r\n".as_bytes();
       // println!("{:?}",check_clrf(input));
    }

    #[test]
    pub fn test_int(){
        let input = "1\r\nasd".as_bytes();
        let (r, _) = decode_int(input).unwrap();
        assert_eq!(r, 1);
    }

    #[test]
    pub fn test_list(){
        let input = "*2\r\n$4\r\necho\r\n$3\r\nhey\r\n".as_bytes();
        let (_r,_) = decode_resp(&input).unwrap();
        // println!("{:?}", Encoder::encode(r));
    }
}