
```note
[^def]: xxxx
    xxxx
```

```note
[>]:
xxxxxxxxxxxxxx
    xxxxxxxxxxxxxx
    xxxxxxxxxxxxxx
    xxxxxxxxxxxxxx
```


Footnotes


Here's a simple footnote,[^1] and here's a longer one.[^bignote]

[^1]: This is the first footnote.

[^gnote]:
Here's one with multiple paragraphs and code.
Indent paragraphs to include them in the footnote.
`{ my code }`
Add as many paragraphs as you like.

## \footnote

引用, 然后显示在下面

在 html 非分页场景下和引用一致

- 分页的情况下显示在该页面下方 (pdf)
- 非分页场景下和引用一致 (html)

[^_脚注, 2]



## \cite

引用, 然后显示在最下方

[^.引用最下方的元素]

## \sidenote

[^>]

[^<]


##

[^tag]: 开始定义

\labal[H]
$$2$$




[:First Term][Definition]:
- This is the definition of the first term.

[:Second Term]:
- This is one definition of the second term.
- This is another definition of the second term.
  The HTML looks like this:




<theorem class="theorem">...</theorem>
<theorem class="definition">...</theorem>
<theorem class="lemma">...</theorem>
<theorem class="proof">...</theorem>

theorem, definition, lemma, proof

<proof>


</proof>

<lemma>

</lemma>


< theorem[勾股定理] counter="counter">
若 $a,b$ 为直角三角形的两条直角边，$c$ 为斜边，那么 $a^2 + b^2 + c^2.$
</theorem>

<quote>

</quote>



<fancy::quote>
\set[]("a")


</fancy::quote>

<tabular[ccc]>
cell1 & cell2 & cell3 \\
cell4 & cell5 & cell6 \\
cell7 & cell8 & cell9 \\
</tabular>




其中 body text 是字符串类型

执行顺序没有规定, 由作者决定

原则上
先解析 patten -> argument
然后解析 body-text -> body-structure -> argument
最后执行 argument

\cmd[p1][p2][p3](arg1, key2 = arg2) {
body-text
}

\cmd[p1][p2][p3](arg1, arg2 = v): body-text

<cmd[p1][p2][p3] key1=arg1 key2=arg2/>

<cmd[p1][p2][p3] key1=arg1 key2=arg2>
body-structure
</cmd>


```\cmd[p1][p2][p3](
    arg1, key2 = arg2
)
body-text
```


\documentclass[article] {12pt, letterpaper}
\usepackage[std]

\date: February 2014
\title: First document
\author: Hubert Farnsworth \thanks{funded by the Overleaf team}



< document>
First document. This is a simple example, with no
extra parameters or packages included.
</document>