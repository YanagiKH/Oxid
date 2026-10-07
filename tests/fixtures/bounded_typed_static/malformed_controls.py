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
# The selected 22 negatives, final-byte truncation, and trailing witnesses also
# ran through both actual semantic consumers in the separate 96-I/O-run report.
LINEAGE = {
    "boundary_results_sha256": "b2e6d70f831a45da0b2545ed4c3caadcac891c868326edb1c7ce2ea988ab466e",
    "boundary_harness_sha256": "a635cfc7b70ad6392edd2f70df82601643286272dbd4f9a0185a785970b931ce",
    "io_results_sha256": "362291134eeff50294c784f2d1da2e066bc30da95afe7677dfd0d6c2a3dcbad1",
    "io_harness_sha256": "e3cee7794d83991caaaed1a986d69cbcbca7c0b9045001dad2571b086551f32e",
}
PAYLOAD_SHA256 = "a2cb0682a78830ea3b85d9ddb2d2b0f3989cf9e24bc622ec842e56126704d0e2"
_PAYLOAD = (
    'c-rmV>vG$+5(n^i@#m_>0$5!7DxEW(#bV)TjxBj~X-=locRx${CfiaPWs-'
    'g{_s@}KizFP;48@;T1pd0Zqua6G+)2Os?f1W~c)i;04?iHfU#^y``~B{ZPx8O3-'
    '>xEHz!(60N=dv+$roRJ4>H~77C^uEdJCcD9tXMz0o+rn(={1fEXbqRs5c&5-Rx3yb2+`0;Jgdb&u#;NKJVzlVTy-'
    'oZhL^UOuK<2{oAucCoa!&6HG_HWIKdqwt0H>YkKRm+ydeGH4*fCuwBA3zg#_0WBZ9bCe=nxShz)BJ76PE1A-uq*u2D?he-Az%K+~'
    '(LrWzW7qXsJlP*D}k+!&^JJFW*tL^e;C4I5}Q#Rvv{dvLb+x^{Y;TGVFF1Ow+Z<Z@va_nz!7qs0i+Iqk0wE42w$e&C3e9g3#?e*6'
    'i`c+qitG|CaHWVn53{i$CLzKl3Wr(u;_>5tQGDI1oEQYAJh^o0Jj=)L4<w!%hF<JqR%)xi@l;g;$RfjNgE0jhakP<tN=m_FBAPN>'
    '9@kb!)(9lUlVey72LzE%PVu&(CSuBPqLzLwwoDmwL-XThyZ^Lt<SR&`@sFa3HJa-;dM#+wQp~#gXQ>{rCtWE;gm}7{7-'
    '+(CpqbgDUY$eJNWr#9FS^h0V&GGcX5M?n$8KMkP7DLo4MA1mlVF)Jb)q|sfz-'
    '_8axiZ*~T%fZ@=OnW<4<q^1D^MupNP2id)N)0<TmC7}b)yU1^lsgMk4l0@C8Ls2$zt^;qmsp_WK=RLS&T|=sf0OU&EzYpA%r-'
    'G##{!rP@w@z!%-'
    '_sM#?a<%duh4rFCFXK(T+J(t5vJtjA(Qs~cJTce(0|NB(TqpD$3;*P8kF0Oe+1Yw`vtXMnO8pbStJ1C#;E0A(>iy#<t7N%V*SEhv'
    'v-0w1C0-fJpqrbZ=umhL2`;##Z8H_j>KRzW95$S;6W2FYeCeX+f#)dgaDwcbf0B|OtH1o8`!atRc{bFO;tAHWp#Uz)1Ub-CzwoG_'
    '~c66g{!^Z*l}v~g9V&Uu;dbw28RKh4j6dNiZn%VGJ4-<Y3J7E^9Hr=q_{*Vo@f+EPd(NsjuM(lg;<JwLUSAhlC$19JLo7^3Ge-'
    '8eXm9YVyCTqxuDUYCnOX(u>+<=8&jE?U6B1UDty6pW}_B6E(_)!4W>jzf*BXS3C#R}16Edj1TD7b$;0o0RFaz37y6Vx5{!5{E97D'
    '}o2qDSfbAAijEe{u*1;hdojU{d?o|xAx?!{+$^T6d*l_fDit#&+GAvAqNZ}@y(s%w`+<5b~<(C!%yRQ=a{yNex2dx11vMh>*Jdoz'
    'CQ8|9{(G_$9`KczkfID?LFX|$1Ii)&NgKTkJ9Quh!q$|OHt9l(7;iriXt6HCCA=-'
    '9Jy1WoCk>?8N0v>+s1yyyXAVdxLqpI_zU041$aBLwh_(<H*dnZZy=oe*o1RNI3t`9&SHc!!WrQ#M!2_xb3G{&Aj=r7x-'
    '7{x!sdVr^b~3gl~rt|N6sA;K)E<wo-1*Ks%O;a2-'
    'jst{7JTptti0ycVq5NJH|R=omJ*$&N{2iJ+Lkq>x^|4W1X?iSZ6WTy|At|MlP(PsNz}@uF_%?p&rXrspAPTQ*BPs$F7&&6d@|R+)'
    'Irq^%vG@lN;%ad)nyFJJ~FDU+yod&7EA|7-x(##u?*`aaNr(##xMU#<+iC-'
    '19QVILk~J7cnKpPJ}j^5YlMWq;{x2s)agv0+)wFKlFrQl;owNo;tSTa1;Ue!Z_Znx7)>YZS7HIZQZ|IV7c>+80ZXi208<sfzCj8^'
    'u2-3VxTk7SzXRzpqoY}V;^Y@Jrk}HDwoP552XXPMp6)<mL4Us#8@)74oMgrR>6*pwY-3?-'
    'LCn+i|vkfawn_ZVza)TI>}#+{Z?&u0qVZq(COYHYdF(eWIx?o<ixI^$V*d4h!gTS1Ybi-'
    'O@EHlcPILgWIa~)hw;jKjAOdF!F9TyS5K0~$3qX29E!q<Jx{2_8Et?GR?}(qLl2S)OMQV&GwnstKQd#6fu^8fCyHp@6Apx+nQel4'
    '_<#Sj=Qn4u%<=d)=Y+?v54}jjVVfACEHkH7BNwt%j;W^sqqV4yII6XFNzf||D2aleJq}%tnm`B1Ee|h{Q%x_lqRq|zK%tk5&z9|W'
    'xw^SPRcG6nZ>0<ZD(U{YQpO*;QWnQQSIQ3KCw$zFNyazyEeX~!$stxL`>2(&SMTtgsg!xElvzIXTRN|geJ3krANUyPj;dg9Um5J9'
    'kCqv8Zj_-'
    'X|0z{KApz9W!xohlYKw^@PW8fF6(>ErQanv4%F%_<e2#t4iH=Fun=Tt=+C42d@?Bk$yl&y&8%d3iMpDCB4@K>!k<`MhhoX))lG?0'
    'KnVv>cOM1G5v)lyr@s!9$QnSo6Hj>)YNNP`YFpI_5bd{s8RYvw9GqxPdRH7*|_2(Eo#X$oadaR+k972!CQ!o9h796AIay*jS^5)Y'
    '{QM6t2k`9AMZ})Atp@W1j8CmUX?ffW01rA<s-$JMn^)_ch&3G_su)(M;HW;<D!Kf`pC?nL<YoZNCZ81VURh}R;ZbaVKUd1uh-Wx@'
    'B1BGiIt<$IjqYp#CF{785y}Bbe=_4Igp7;|jSJD@}zPqQ*a(j`R-miQLOX-}Hiu)18rd~}-'
    'CMA=SNy(&SdxOQKWKuFISxicYORfs!n6gY4Y*ZM&1EZ=UPFL=XqH0n?33F|oB@%#Cl+t6Ux`xlmAN4Z7-'
    'fvhItM!Vuyj)(=zx?YpPhVc;&J>h<^23udX=I;jnl1yA1c|-NK_8ngi3-'
    'ytPpRfGT~?L&FfUF^=cG%6DnZxR!y>u*j#JaIGue(~u~;k?i^XEGSWb+^OcjVAMNYn!5F!r_Rq?_xAqNa$_(n_=G`(gn;?Z$S16x'
    'QwzT?z>wUf==az(p`I?~i3#pSd*)ygSm;b~(P_%YW#St-g&QC5nYZ>$t$r6|kHW2Go7MOn;F??1Fh;(?;)uHJx}i&Du2?mD-'
    'P=3tDTu)6B&Kx!%UL*MEGRWLB1ytC7-'
    'tZsInz}L&c^t|dRC58Ayx`zN`k}=7cWHBZglPnfvk}=6*`8tz&r7kieb;eOV){d#hI0B~F-'
    'H1*VkwW6!ySSblr<~Org;LCb*k72mzNf{Y<?VhWhry%v+Y6xde&wG?r6(YOQOT%eRI(VAj7k=ZQOT%ev7Dh2<tpA2bQh{ekWfmXQ'
    'jkK&#%@S~);nVIIXICL+GsGC39=I(j(XBte_pUUXm?pP>3FrD&!a;n35-'
    'X^Bjb_9cw{`XSd2%;Ba7udkEkMIbmWJ3QwQR48b!9c?%VXaJ4vy#55;2~qvessLocYaV@r9cP0oDaZgIEV-qDUfeecKao*OttrZZ'
    'xC|LhVGC9Z*7^C9cgY~7U*OYP`KDU~CENQRL+4%RK<iVTG!j|i^Sqdow4oBfIpmrl`c5ifXFuU9<NQTn;=|MFV`rmF1oxg8JotGW'
    'rHp2Q$32D4%?FgoHf-yNSlD+J?w2{gU%;bF4OH(5{q(6w1_1A809Vi=rv!t<-'
    '5#bOkEpr9(xgA*!JGBQAo5wTV9l`%9`2s=elQ5}sGp(qJybP2V$9z4k@ERE%gZWry#PL9bmX=y8WwAw9sA*<cy%k^@-P!a7m>)Y-'
    '19o>%gMs0L`s-'
    'f$R4B5zv<$Am8%hj(<+k7XgVxlrpnW!u#Dif8(VxlrpSu8*Gg%F}YF_y3EeN=?%;;4{Xk(!yaL`;Jid&`li1}>R>W5J}eL*bBBEG'
    '@Kg>?&H`E>|~;-'
    'RJe<^K!S6?N+wmDXUXWown6~^cU=R({p;f*z1qOGu&SN_WNI^EK`;#%VNqhWmznyEK`=n@)L8Sa^*Z{=p`UZb4)<7hCU>@rpO8;='
    'sZFrp~6bEK;)ugq+v(O9P5j)maCoI$mRl4HQI_oO0FMQJzJhS38ngNP@0CS%9!vhmg*Ch`iu_^?4z2J=Z&SLlVQoQH18OeEQTe+l'
    '3~eWSbBw}8ijL*D%0g$(}(+rD{yJjN^zZwE1jCS*n3S?QI%_NB{+#AckvyL%28kL<qz3x*5Bo&m$lDSo&Nqmmy+PU'
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
