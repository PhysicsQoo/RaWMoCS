#!/usr/bin/env python3
"""Pinned QuPRS Python boundary integration, opt-in scoped adapter only.

Uses the actual QuPRS encoder, registered external adapter, and process
supervisor. Does not modify QuPRS, its Rust FFI, or its default solver.
"""
import argparse
from contextlib import contextmanager
from fractions import Fraction
import hashlib
from itertools import product
import json
import math
import os
from pathlib import Path
import shlex
import subprocess
import time

import symengine as se
import sympy as sp
from QuPRS.pathsum import F, PathSum, Register
from QuPRS.interface.ps2wmc import to_DIMACS, run_wmc
from QuPRS.utils.precision_count import PrintedCount, _number_and_unit
from QuPRS.utils.solver_adapter import (
    SolverAdapter, SolverCapabilities, SolverExecutionError, SolverResultError,
    execute_solver, registered_solver_adapter,
)
from QuPRS.utils.solver_process import run, SolverTimeout, SolverProcessError

class RawAdapter(SolverAdapter):
    def format_weights(self, real, imag):
        return f"{real} {imag} 0", "1.0 0.0 0"

    @staticmethod
    def parse_protocol(stdout):
        lines = stdout.splitlines()
        if len(lines) != 2 or lines[0] not in ("s SATISFIABLE", "s UNSATISFIABLE"):
            raise SolverResultError("RaWMoCS incomplete or invalid SAT status")
        f = lines[1].split()
        if len(f) != 5 or f[:3] != ["c", "r", "mpfr"] or any("/" in x for x in f[3:]):
            raise SolverResultError("RaWMoCS invalid count record")
        try:
            real, ru = _number_and_unit(f[3])
            imag, iu = _number_and_unit(f[4])
        except ValueError as e:
            raise SolverResultError(str(e)) from e
        sat = lines[0] == "s SATISFIABLE"
        if not sat and (real or imag):
            raise SolverResultError("RaWMoCS UNSAT with nonzero count")
        return sat, PrintedCount(real, imag, ru, iu, "rawmocs-mpfr")

    def parse_output_precise(self, stdout):
        return self.parse_protocol(stdout)[1]

    def parse_output(self, stdout):
        p = self.parse_output_precise(stdout)
        try:
            re, im = float(p.real), float(p.imag)
        except OverflowError as e:
            raise SolverResultError("count outside finite binary64 range") from e
        if not math.isfinite(re) or not math.isfinite(im):
            raise SolverResultError("count outside finite binary64 range")
        return complex(re, im)

def adapter(path):
    @contextmanager
    def resource():
        yield path
    return RawAdapter("rawmocs", ("--digits", "60"), SolverCapabilities("pair"), resource)

def no_other_solver(_):
    raise AssertionError("Unexpected fallback or implicit solver resource")

def exact_cnf(raw):
    """Independent exhaustive rational oracle on original emitted bytes."""
    lines=raw.splitlines()
    _,_,n,m=lines[0].split()
    n,m=int(n),int(m)
    assert n<=16 and lines[1]=="c t wmc"
    weights={}
    clauses=[]
    for line in lines[2:]:
        f=line.split()
        if f[:3]==["c","p","weight"]:
            weights[int(f[3])]=(Fraction(f[4]),Fraction(f[5]))
        else:
            assert f[-1]=="0"
            clauses.append(tuple(map(int,f[:-1])))
    assert len(clauses)==m
    sat=False
    real=imag=Fraction(0)
    for mask in product((False,True),repeat=n):
        if not all(any(mask[abs(l)-1]==(l>0) for l in c) for c in clauses):
            continue
        sat=True
        a,b=Fraction(1),Fraction(0)
        for v,value in enumerate(mask,1):
            c,d=weights.get(v if value else -v,(Fraction(1),Fraction(0)))
            a,b=a*c-b*d,a*d+b*c
        real+=a; imag+=b
    return sat,real,imag

def check_count(actual, expected):
    for a,b in zip((actual.real,actual.imag),expected):
        assert abs(a-b)<=Fraction(1,10**45)*max(Fraction(1),abs(b))

def executable(path, body):
    path.write_text("#!/bin/sh\nset -eu\n"+body+"\n")
    path.chmod(0o700)
    return path

def expect(exc, fn):
    try:
        fn()
    except exc:
        return
    raise AssertionError(f"Expected {exc.__name__}")

def main():
    ap=argparse.ArgumentParser()
    ap.add_argument("--bin",type=Path,required=True)
    ap.add_argument("--out",type=Path,required=True)
    a=ap.parse_args()
    a.out.mkdir(parents=True,exist_ok=True)
    binary=a.bin.resolve()
    assert subprocess.check_output([str(binary),"--version"],text=True,timeout=5).strip()=="rawmocs 0.1.0"
    wrapper=executable(a.out/"supervised-rawmocs",
        "exec prlimit --as=3221225472 -- "+shlex.quote(str(binary))+' "$@"')
    report={"status":"IN_PROGRESS","binary_sha256":hashlib.sha256(binary.read_bytes()).hexdigest(),
            "quprs_revision":"b398f07d6845efc23540549ccbfec717c27b1722","cases":[],"controls":[]}
    def save():
        (a.out/"integration.json").write_text(json.dumps(report,indent=2)+"\n")
    save()
    real=adapter(wrapper.resolve())
    reg=Register(1)
    identity=F(reg)
    flip=F(reg,data={reg[0].name:sp.Not(sp.Symbol(reg[0].name))},bits=(reg[0].name,))
    with registered_solver_adapter(real):
        cases=[
            ("identity",PathSum(se.S.Zero,identity),True),
            ("diagonal-phase",PathSum(se.Rational(1,4)*reg[0],identity),True),
            ("bit-flip",PathSum(se.S.Zero,flip),False),
        ]
        for name,ps,sat_expected in cases:
            cnf=a.out/(name+".cnf")
            other=a.out/(name+"-gpmc.cnf")
            to_DIMACS(ps,filename=str(cnf),tool_name="rawmocs")
            to_DIMACS(ps,filename=str(other),tool_name="gpmc")
            assert cnf.read_bytes()==other.read_bytes()
            sat,re,im=exact_cnf(cnf.read_text())
            assert sat==sat_expected
            precise=run_wmc(str(cnf),"rawmocs",timeout=20,preserve_count=True)
            check_count(precise,(re,im))
            observed=run_wmc(str(cnf),"rawmocs",timeout=20)
            assert observed==(float(precise.real),float(precise.imag))
            raw=run([str(wrapper.resolve()),"--digits","60",str(cnf)],timeout=20)
            assert raw.returncode==0
            actual_sat,count=real.parse_protocol(raw.stdout)
            assert actual_sat==sat
            check_count(count,(re,im))
            report["cases"].append({"name":name,"sha256":hashlib.sha256(cnf.read_bytes()).hexdigest(),
                                     "sat":sat,"real":str(precise.real),"imag":str(precise.imag)})
            save()
            print("QUPRS_PYTHON_ENCODER_PASS",name,sat,flush=True)
        for name,raw,sat in [
            ("sat-zero","p cnf 1 0\nc t wmc\nc p weight 1 -1 0 0\n",True),
            ("unsat","p cnf 1 2\nc t wmc\n1 0\n-1 0\n",False),
        ]:
            cnf=a.out/(name+".cnf");cnf.write_text(raw)
            p=run_wmc(str(cnf),"rawmocs",timeout=20,preserve_count=True)
            assert p.real==p.imag==0
            actual=run([str(wrapper.resolve()),"--digits","60",str(cnf)],timeout=20)
            assert actual.returncode==0 and real.parse_protocol(actual.stdout)[0]==sat
            report["controls"].append(name);save()
        invalid=a.out/"invalid.cnf";invalid.write_text("p cnf 1 0\nc t pwmc\n")
        expect(SolverExecutionError,lambda:run_wmc(str(invalid),"rawmocs",timeout=20,preserve_count=True))
        report["controls"].append("actual-unsupported-input")
    # Fault processes isolate the existing QuPRS transport/lifecycle;
    # not evidence of a real solver numerical result or hard RSS limit.
    for name,body,exc in [
        ("partial","printf 's SATISFIABLE\\n'",SolverResultError),
        ("unknown","printf 's UNKNOWN\\n'",SolverResultError),
        ("nonzero-with-count","printf 's SATISFIABLE\\nc r mpfr 2 0\\n'; exit 2",SolverExecutionError),
        ("unsat-nonzero","printf 's UNSATISFIABLE\\nc r mpfr 2 0\\n'",SolverResultError),
        ("nonfinite","printf 's SATISFIABLE\\nc r mpfr NaN 0\\n'",SolverResultError),
        ("timeout","sleep 30",SolverTimeout),
    ]:
        fake=adapter(executable(a.out/("fault-"+name),body).resolve())
        expect(exc,lambda:execute_solver(fake,invalid,0.2 if name=="timeout" else 5,
               resource_resolver=no_other_solver,process_runner=run,capture_decimal=True))
        report["controls"].append(name);save()
        print("QUPRS_PROCESS_FAULT_PASS",name,flush=True)
    expect(SolverProcessError,lambda:execute_solver(adapter(a.out/"missing-binary"),invalid,5,
           resource_resolver=no_other_solver,process_runner=run,capture_decimal=True))
    report["controls"].append("missing-binary")
    report["status"]="SCOPED_PYTHON_INTEGRATION_COMPLETE";save()
    print("QUPRS_SCOPED_PYTHON_INTEGRATION_COMPLETE",len(report["cases"]),len(report["controls"]),flush=True)

if __name__=="__main__":
    main()
