## Balance of

Instruction count

 Tx         | sol | wasm
 --         | --  | --
 mint       | 54  |
 balance of | 210 |

Cycles

Bench          | Total         | load          | exec          | ops
--             | --            | --            | --            | --
fibish 4 wasm  | 6,634,352     | 63,580+29,867 | 138,438       | 1632
fibish 3 wasm  | 6,622,117     | 63,580+29,867 | 126,174       | 1498
fibish 2 wasm  | 6,588,536     | 63,580+29,867 | 113,910       | 1364
fibish 4 sol   | 5,451,732     | 4,470         | 68,045        | 891
fibish 3 sol   | 5,440,575     | 4,470         | 57,002        | 744
fibish 2 sol   | 5,429,396     | 4,470         | 45,959        | 597
get name wasm  | 6,794,425     | 64,165+30,348 | 27,686        | 346
get name sol   | 5,998,213     | 25,302        | 18,614        | 220
balance  wasm  | 14,661,091    | 128,330+60,696| 3,146,569     | 26,265 + 14288
balance  sol   | 6,078,704     | 24,945        | 56,885        | 399



## Notes

- Dropping logger takes a non-insignificant amount of time

