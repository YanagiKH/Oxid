"""Fixed byte-for-byte AST1 negatives and I/O inputs from independent review.

The compressed literal contains only retained input bytes, their original paths,
SHA-256 identities, and observed seekable-input consumption. It performs no
parsing, resolution, type inference, or construction of semantic expectations.
Compression avoids repeating thousands of inactive zero cells in source control.
"""
import base64
import hashlib
import json
import zlib

# Original evidence: 59 controls (seven closed-validator baselines + 52 negatives).
# The selected 22 negatives plus seven focused-validator negatives, final-byte
# truncation, and trailing witnesses also ran through both actual semantic
# consumers in the separate 96-I/O-run report.
LINEAGE = {
    "boundary_results_sha256": "b2e6d70f831a45da0b2545ed4c3caadcac891c868326edb1c7ce2ea988ab466e",
    "boundary_harness_sha256": "a635cfc7b70ad6392edd2f70df82601643286272dbd4f9a0185a785970b931ce",
    "io_results_sha256": "362291134eeff50294c784f2d1da2e066bc30da95afe7677dfd0d6c2a3dcbad1",
    "io_harness_sha256": "e3cee7794d83991caaaed1a986d69cbcbca7c0b9045001dad2571b086551f32e",
}
PAYLOAD_SHA256 = "449b88440da8e1f4d781956d2ad1cb37a5f56c0183fe7ff8f4cb3a386f3d2f69"
_PAYLOAD = (
    'c-rmV?Q+{VvIgLL$?w$$jc?4YY@M2FG#ZX-ZOSFdlg*jhdp`}zwq#q%L{pimI%2;neb^%Dh+-'
    '%_Y7+SU(*xf3&Gv!Yr(gc{`zP8=yQlew5X?V5t*6JQ{jXp6Kc9a2<RQ4=9e|B7vNbW<Y>RD<$9<{}<gt~jb2T;JgYn?NyrdYJlfd'
    '{KsJG(fN}Y;XT(Yb#BiHPVHO||`tpJeM?Two+e!eVi3ox3o85orJo=h-'
    'o(jr&E*!xA@AT*23%cozJYg^<RaIc?<Adf*e3C;5I>4h4*FK9y)ZKN17Rc}j!P|;!s$02lIE&G%@bFwaR2Wt{RjX7oGlC-'
    'K(rodcLmN?-9%aWd^-TH3gw%Yv0+kU_Kx}wef)5EkfE3jFn+icf&>q(~UPxtpL-'
    '0fF&^E5Twep_w%w>5wLOt|CS=kG7*pE4tS`tLtZ4F!TEMU)~+5v5T?DWWt#KfNoW6j6#OjUwtjqDm@}JWymX$zvC8^qPS@k+Y30C'
    'f`%6#RTZ7W{f>`h>;BWU=aKdAPQDCgr9+^c|m6p6|z-'
    'CDWVin8by>MN~2LkDWWugL5olk^#M_AZ0(jJ`s^tcgE`kw*;1okxaVZ3W%N`C5|$EW!eZD_@yR>S;Rg_9e^w>RUaUkZq7+ezD9zu'
    '6s3kTZ6j2&Qlp;zIrBOt^K@|1~4LXO2tynPF15h1xDU&<do-'
    '#DD!5B`&)k2RpwgTuf8sz4Vh+0ob`}J@9S~t4FO&{j{`>4c0siag=DrvOdq*T%<m6S?KC5=+)J(WU=p(L~=h#?p$xk{9KsIEYTz%'
    '}$zh*Q7>J(=WdXerkQ1Oh0sM=EWe_Nz@_ZE?Ee)xXzMTRrn<yZL&Bn!dM~zYkDm@wFzafHDdwjRHymrBOgBpcGIV1=M>$iIrIP2v'
    'D8y$VadqT57GtEM}^hlO=A3eaxoT5^bdvU8)7-'
    'LIKhdP{JVI?zpXXk2qZ+rZ@AQB~lC*I)(^zBvQ%_Jy=Rb?(G3gUjC)A$g?J8c|<WJaX<u_!n@{R07@1vYLsV=&s%vu$n$nQpX~T;'
    '3Ubf$^oPHgUQp&^swu@Re@Et*zl5?R7klL7<#$5Q80PuxSW<*o&#?{A$g3gnmfX0KGr>1-'
    'KICXz?yvVUop(Yz4&#=5y|qm=gMk4zA=^msLDoc~<clf3QZ?ke7*|@e#Re-'
    'D4!8CC3FZ$eJEM*BXtrm$B)O1Ws$AH2nJAS27J^*jjco${&Byci*cxwkPwnLW4&%MHXo|eg1Q9Zz>_osiJ8$!{{~|~(xLf>k=k)8'
    'EEPx$NSDJt7ryIvuD{{NQ;{!Ac$lLuJ&$rKfgXjMSaNA$&rH|icy}yU>?spo^jk8S{LOs{g1NZ_IdW~MtKv2P8!xETV=miZ~YeP>'
    'BGp5v$ZBL;w9NE^NCfctz)9QXLM5AwX&sX5>%-l*iCET(JXMTWi=C%oElyFKoC7ebHr-'
    'W0&X_Rp931?aqCP3oeYcYwVsTisO%Fv=K-'
    'W3wD<>o21AOK3)$n;c@Dimo^Um{%NJ<=E6t#&K`YkwMZ@0O#iQ`Tu^ZsDxc%G`l<PFbg{(<tkdb;>%8vhK*bS}9N_5k+BBBU9n(D'
    '>F8qN2QJiVn%IF&_|}_Rs|s<yVP>=F}5S?WRY8Lt4G|*&wJjk_TL__sm-'
    '08Um2&2Q^qOdlyO>}Q^skOamu(qV%+OA$~etJ80SNbf*lXFSH^(7V&&4HST7c8Xc0{6CfF_whMuG45~S%^vk5&jm?Pt8yV>nlueG'
    '&jmDNrAc7^3GFGNA7pi|H(=oEAcx|7EWI*o!(L8o;&je>5BOh(pY@6r-3><Z;VJ$0c&s1-'
    'Q}fRdZ%NbG%1R2xL5P($I+0)>)~psV*A`uA$L$2~vrv|nvE_oI{isXy(+W>=u@`wKeXTV!z;dW&r5dy5?Ga)P`#Izk-CBj;>!HCF'
    'lAk4I<vkR;hxw!?U3&HFyCrc)XB%j!uoTR-<8NiGYl$kIS%SkMX>uo}(kLl2SyOZk9}3++YFJ~Ly1fhwRN!-'
    '8np5@tf+!d5|<|KGoC`PFDNOWgl0Iqv!P(2FF@>qH5qSvajK8ke|`k1clSwR-uD!BR@&2(4g;9MMVZvCDK=85(e^sau8=OFYsFw|'
    '7r7g^nkmt?T`Iy1PPE7niZzN*TByr2FGa8QpZH%=bU8l+EKO+-}Dt{RjG%1nrn)j#kQUwNm!x8=ec5GOLv`%}sx$^LF2Nwo-'
    'P(`#^J21$+O=poczc7R<SxyEOh|ED#tOf;11S7gmTZ21ktOg_<aiTQWIY94JcOxL$4Yt>c-'
    'FNjBTYTVdKGuDATBx+HnKguibjH98qdEi8H{YBNSsbBi8|+UrPai*7Q#jHDK&xfB+;3d(bnNJmoBEHgTi+RI34FLf}DM%nZ!d0Pt'
    'iWL=_Alh30>lX>*#=q&n<9cr+lxMGrX%?BF2^ov?T?<M8ak<`|AU-p8c-'
    'G<gU4<5aHs{1X@61rw&weKzGXA#PgvvU0rLUk|KB@=4FgHeMHMy=7osErOrtx-ZLp<X@{buem;66&S$1g=s=w6?UuhOo3&(Ua+cs'
    'ia;T_R@p5t}_^-VA&@t?nsqek0+HU`hx3;+ln?1kGNg$u5#1Ina^P<UXoHaKcm>xib_ePq*78Tsg!hU(5RGDN-'
    '8CdN@+gjDns%y@i<|lK(`Gjh${RzbEjuflMsrDN^Qg*0k{Ot&ATFV*c9zaFY}wHE%9pFOt_=<`kMab-'
    '_Cjd^eS_qpk$-%7KKSY*;wK@4U8j1vL-qC-8d!6jZ>atNp7506={CXkEv6XsX>$=^UH1#P5Hp7vG0t!;b=4(jYgx<Xf&ELV=-'
    'd^M2McEE!jCwom>&TkdJ{}aIV{mgy<x#B+6`qp&EOrE?WPAQ%}>Lw-'
    '4(H_lG*t=#b)iTAf?UIcA0P#>((>&U?~Ql$N5j6t&!FDN0LGnuSM8QCf=9sGU9@w1;c~y`?7JfReIM$vA5AZ0*%R?=6O6iY+~0jj'
    'qjot1}c1jv(*{JKghixBmjRTo0z#S<flS`Jd7~1Spe~Ny;RRGD(@F(I}IYNgB=fnbZn4=I*gk=-'
    'GzSLM+~Q7ou;bdn1a7E>dcZO^b$NO5%--'
    '(I*I@9htOw#8t=j{nM7`!K0pbS3v3G%zq)3UVs2fC8d&5NuyL!Drq!IC8d%^bAd{j3R{cNj4L((yPO^79MH8;$#l+8TMHrD<P39m'
    'wO8mw2+6Q@Cq3zHzOG0dw7;&JbUNE#&!f4L1j-}jk@85RJW?KMG|D68kw){8M_57#-k|N)j1I*87)7?2X6rcaj-zj6UADpZUQ<tO'
    'q2(Y?`x;Xhn_M{HZuPL<J>Z_c{Mp0qUKenVOc%uT@zq5{j8q(&YF(0-S)0ku=hA}hIT!K>%n5pGp_8l$ler5Nse3TBob&;>-'
    '#$$=pE`y6mA~Ruy`AwwN9pDQ0nJ4M*2AE`ANZHXczdt>%YKpcJMhmu^N*G}*8C~|obLJ-8-'
    'C^AQj^S$4?e2V{4oEP`B`|%KaEEDcj&9acZAYi553h~3pj%S5L)Y4LoT+M+B}AuQ!B+shnAD0NWiW@3P&JKo9TafyIJjc+OH?RKP'
    'A%8(afaZ@P6HJj2QK~Z|G?^dZL(>^nUIO{r{B<{r|P|PMVwlvz6qx=$tLC+LrRTm8n4P;rYJJQ<nKRFnR_|A%=6p0x~ksL0Byan0'
    'v_70kcRe#NvE`=(~}XGvCG#vfPWQQv-if7R%O)u)I5r<V%jcw-hLtAlpYYkrE|odu5X;`%GnA%q^{f@o7?}U40zMSGEcHIe2;Xa!'
    'Ka!`p2stw>V#J{WJLRNqZ<Z9?TI>+Tpj@xXSWs)5}P{AbSSWAdD|zpqMVZjwZX52+7KR6%ge8hg<tj=4G261vZ@jYs~LIq|w@vS3'
    'GX}^V;%uxf0dfy0Au?St2sr^B(_YBws(T1pDFEy}Q?FZdh1YOE#FYv<-km>7BF7)j>y_b1yBD4JCOK!Ff-'
    '`n$j^sXK_UnY@PIIakt$(J+A1>`o4*i?)!?q(LG=1tn<rIZK<|YTdJ-9C)z5iE!CDrwWZp6r!A-'
    '2(kxk9B|s!oUGBn$1}G1lqw1lAnhS8FY&tg#SPVIgfm=+zRHV_C;FG@BAGh4s|5<(E_1%~Kb;x=%>jhh-'
    '3%07~Z57kFck{hWMPjTf!lF3NavD9e%gat<j4P7oz06tU(NI?ybDeRO><w&vc9pQRdEXDm3pPKF|D2^2^8$_@%wv_wRb(|pF`HtD'
    'igKPUk#GIjx$*gdX1Egl=s0GWs^n)&J}YV&@y{&_J3n67F_Yqiahza>v2V!WWgj(86zM<!)-'
    'K}sxp3m<vCR|Y*XE|SoixjG)%a*w-f`nE#|fHQR4=EC+2==UD9s{W<yzq+tiqH^FG!WVwH=+0l}jU!p;1EX3<Wf~`%t~jg>9;-'
    'TT#i;RfIyPuiMRZw;B(bSNqMsdAd$m7Z&n!&aM(SnNg)Wb9Plco?X@Aa?h<goL#lxbF1E-U8PxWSkyp^?ph@-'
    '=KBGS<_FZYz_{o2pgPj1nqJkEWA8ZuP!FLtYcY)bY`t$`bTBBbhh{86V&GA1$u49<8{IhAt>Q`FgA?D0gMQ=H^z=}9E9#jx(^~Aa'
    'l0Wu%<w1adu(0zLD|g{&K!2%XRpd7&WqCv~ByYzNj_u&87!K7c(P4^Nrj#jU+T&a(k8H9s9mM)}j_6{KoV-lQFGmf|m(g*FPeJba'
    'Y{knN`L`bIRva}7CJv}(1}C>I$V#UwS9xP@-'
    'EL&bjf$7?6iD78UviNvX7uQrX*bSVu^R~So0*Rr4D%>#1r8nM(R?J?sDUgYi|!KA)*(0W<K4E|R46tXhGu0$u*>K#BFuh!sd$kYM'
    '^p8~*WaM6TFr9oHa_xBx(9{jNak4aI^M&Z-)S_9vl~zY@YZE;+zOfEtY?hiO-o%^QhGyQZGq;j^R>m%m0*F2IRaBmCl#;FwBK&-'
    'SJ?LhTbrq^2g<Ijc%5I!Ijg-ZcYS<m;v+LoA1XplAn*ly?VfTevDad~O~@YDM_+1jp3s+2<fWw?tOzNb>lM*l_i|2Vyil}@7m9Z2'
    'JvU=<&W%Fj2>_*a@7&n~AEn77%aAs{696Lh<!<tHI97DI1Q;c}`2;fE{Vh02$P*UZF`9l2pwVav&%7SSqUh5o0$)1su)$j}jG1!^'
    '1W>#Wp%$=(f+#9RGNzX7sQ2h0``)oPcBNHeA@VsX_4S1JtNLyKp=!6|2b}h6TCrH~+voLW^{l?#=Lfv+o2}UB^SK3m-'
    'g4(HPsE>h`?j9`S!J0YM3q%kDk>F~Mn$Ef(r8pvDk_cUFa00{Z$G+9nh33z0LA!TAlA$!5heB^cBIg1@`%MzPGl=_i1M_{<dTS`y'
    '4p`&h3otEbhp}n-K@T@_Y?1Sy!(@~I=86vw)!9W15f+$n!3#NwB=vtSGfE1%fEhCWvQ}MSsGQADodkLWvQ|>n!hk-Dp$r*f|gwfT'
    'n!<hFRpctrV6qgaxgYPMaE3yFh@k=1tT@IfQfuL3Tr*>`Hr_&kgDR21ycO^dDe^Nsk2ZjKL(}m^%_58sk~sREqK>Jqt|#x%f`}tm'
    'Y41%7LTQ6N3o<)EGd>0OB%(}+g{^6Q)*B+^~B@gKBNMaE7zRO=rTSxtZaO1B^E(dDy?Q`*!R@fHaICqeS73zdAr^GDK8!8zEE}g@'
    'BaZc(=4+'
)


def records():
    payload = zlib.decompress(base64.b85decode(_PAYLOAD))
    if hashlib.sha256(payload).hexdigest() != PAYLOAD_SHA256:
        raise ValueError("fixed malformed-control payload changed")
    value = json.loads(payload)
    for group in value.values():
        for record in group:
            record["data"] = bytes.fromhex(record.pop("input_hex"))
            if hashlib.sha256(record["data"]).hexdigest() != record["input_sha256"]:
                raise ValueError("fixed AST1 control bytes changed")
    return value
