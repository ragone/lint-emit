<a href="https://travis-ci.org/ragone/lint-emit"><img src="https://img.shields.io/travis/ragone/lint-emit.svg"></a>
<a href="https://github.com/ragone/lint-emit/blob/master/LICENSE.md"><img src="https://img.shields.io/crates/l/lint-emit.svg"></a>
<a href="https://crates.io/crates/lint-emit"><img src="https://img.shields.io/crates/v/lint-emit.svg"></a>

This tool aims to run **multiple** linters on a commit range compatible with `git`.                                                                                         
                                                                                                                                                                            
Inspired by [lint-diff](https://github.com/grvcoelho/lint-diff) and [lint-staged](https://github.com/okonet/lint-staged)                                                    
> Linters are great tools to enforce code style in your code, but it has some limitations: it can only lint entire files.                                                   
> When working with legacy code, we often have to make changes to very large files (which would be too troublesome to fix all lint errors)                                  
> and thus it would be good to lint only the lines changed and not the entire file.                                                                                         
                                                                                                                                                                            
> `lint-emit` receives a commit range and uses the specified linters to lint the changed files and filter only the errors introduced in the commit range (and nothing more).
                                                                                                                                                                            
# Configuration                                                                                                             
You can add a linter by editing the config file found in your user path.                                                                                                     
* Linux: `/home/alice/.config/lint-emit`
* Windows: `C:\Users\Alice\AppData\Roaming\ragone\lint-emit`
* macOS:   `/Users/Alice/Library/Preferences/io.ragone.lint-emit`

If no config file is found, you will be asked which default linters you would like to add.                                                                                  

                                                                                 
```toml                                                                                                                                                                     
[[linters]]                                                                                                                                                                 
name = "eslint"                                                                                                                                                             
cmd = "eslint"                                                                                                                                                              
args = ["{file}", "-f=compact"]                                                                                                                                             
regex = '(?P<file>.*): line (?P<line>\d*), col \d*, (?P<message>.*)'                                                                                                        
ext = ["js", "jsx"]                                                                                                                                                         
```                                                                                                                                                                         
              
# Installation
```shell
cargo install lint-emit
```

# Usage                                                                                                                                                                     
                                                                                                                                                                            
### Lint the last commit                                                                                                                                                    
```shell                                                                                                                                                                    
$ lint-emit HEAD^..HEAD                                                                                                                                                     
```                                                                                                                                                                         
                                                                                                                                                                            
### Lint the last 3 commits                                                                                                                                                 
```shell                                                                                                                                                                    
$ lint-emit HEAD~3..HEAD                                                                                                                                                    
```                                                                                                                                                                         
                                                                                                                                                                            
### Lint local changes that are not yet committed                                                                                                                           
```shell                                                                                                                                                                    
$ lint-emit HEAD                                                                                                                                                            
# or                                                                                                                                                                        
$ lint-emit                                                                                                                                                                 
```                                                                                                                                                                         
                                                                                                                                                                            
### Lint using `phpmd` and `phpcs`                                                                                                                                          
```shell                                                                                                                                                                    
$ lint-emit --linters phpmd phpcs                                                                                                                                           
```                                                                                                                                                                         

