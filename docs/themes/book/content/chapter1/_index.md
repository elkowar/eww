+++
title = "Introduction"
weight = 1
sort_by = "weight"
insert_anchor_links = "right"
+++

Testing every `elements` you can find in [CommonMark](http://commonmark.org).
<!-- more -->

Quisque viverra a eros id auctor. Proin id nibh ut nisl dignissim pellentesque et ac mi. Nullam mattis urna quis consequat bibendum. Donec pretium dui elit, a semper purus tristique et. Mauris euismod nisl eu vehicula facilisis. Maecenas facilisis non massa non scelerisque. Integer malesuada cursus erat eu viverra. Duis ligula mi, eleifend vel justo id, laoreet porttitor ex. Etiam ultricies lacus lorem, sed aliquam nulla blandit in. Maecenas vel facilisis neque, vitae fringilla eros. In justo nibh, pellentesque sed faucibus nec, varius sit amet risus.

> This is a quote

- a
- bullet
- point

## Some code

```rust
fn main() {
    let greetings = ["Hello", "Hola", "Bonjour",
                     "Ciao", "こんにちは", "안녕하세요",
                     "Cześć", "Olá", "Здравствуйте",
                     "Chào bạn", "您好", "Hallo",
                     "Hej", "Ahoj", "سلام"];

    for (num, greeting) in greetings.iter().enumerate() {
        print!("{} : ", greeting);
        match num {
            0 =>  println!("This code is editable and runnable!"),
            1 =>  println!("¡Este código es editable y ejecutable!"),
            2 =>  println!("Ce code est modifiable et exécutable !"),
            3 =>  println!("Questo codice è modificabile ed eseguibile!"),
            4 =>  println!("このコードは編集して実行出来ます！"),
            5 =>  println!("여기에서 코드를 수정하고 실행할 수 있습니다!"),
            6 =>  println!("Ten kod można edytować oraz uruchomić!"),
            7 =>  println!("Este código é editável e executável!"),
            8 =>  println!("Этот код можно отредактировать и запустить!"),
            9 =>  println!("Bạn có thể edit và run code trực tiếp!"),
            10 => println!("这段代码是可以编辑并且能够运行的！"),
            11 => println!("Dieser Code kann bearbeitet und ausgeführt werden!"),
            12 => println!("Den här koden kan redigeras och köras!"),
            13 => println!("Tento kód můžete upravit a spustit"),
            14 => println!("این کد قابلیت ویرایش و اجرا دارد!"),
            _ =>  {},
        }
    }
}
```

## A table

| a  | table | in | markdown | !!                              |
|----|-------|----|----------|---------------------------------|
| 1  | 2     | 3  | 4        | 5                               |
| 1  | we    | ew | we       | with a longish column inside it |

## An image

![a cat](https://i.imgur.com/t6nPdY8.jpg "A cat")

## An iframe

{{ youtube(id="dQw4w9WgXcQ") }}
