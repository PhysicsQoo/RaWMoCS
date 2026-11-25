"""Independent, source-locked GF2^8 numerical witness (research only).

Exact low Taylor coefficients of the original WMC polynomial P(z) at
z=-1, z+1=i*delta. The source CNF has only weights 1 and z. The coefficients
and N were computed using GMP integer modular arithmetic over all 64
disjoint, exhaustive cofactors (eight batches), not MPFR or GPMC.
This is an independently computed numerical enclosure, conditional on
the source-locked enumeration, not a formally checked structural proof.
The witness is not used to compute or alter any solver's output.
"""
from fractions import Fraction as Q
from hashlib import sha256
from math import comb
import json

SOURCE_SHA = "1a8916f10cbe6b58eb28b5e060429722c8aebf69eca80749571667165aab3269"
N = 2**174
D = 241
BASE_BITS = 223
DELTA = Q(3061616997868383, 25000000000000000000000000000000)
COEFFICIENTS = (
    39768823762042841331134365696,
    13075741664951829153621999616,
    282869298976709392814449885184,
    15336703747014779387257682919424,
    -368260604004995337608779511365632,
    7626599308860831371909381633343488,
    -126828741435564734944516018305761280,
    1781143815587856376224952722755944448,
    -21349496296487347679127966672540925952,
)


def enclosure():
    k = len(COEFFICIENTS)
    # Because the polynomial's histogram coefficients are nonnegative and
    # sum to N, |c_j| <= N*C(D,j). The packed balanced digits are unique.
    if not all(abs(c) <= N*comb(D,j) < 2**(BASE_BITS-1)
               for j,c in enumerate(COEFFICIENTS)):
        raise ValueError("INVALID_GF2_PACKED_COEFFICIENT_BOUND")
    real = sum((c*(-1)**(j//2)*DELTA**j for j,c in enumerate(COEFFICIENTS) if j%2==0), Q(0))
    imag = sum((c*(-1)**(j//2)*DELTA**j for j,c in enumerate(COEFFICIENTS) if j%2==1), Q(0))
    ratio = Q(D,k+1)*DELTA
    if not 0 <= ratio < 1:
        raise ValueError("INVALID_GF2_TAYLOR_TAIL_RATIO")
    tail = N*comb(D,k)*DELTA**k/(1-ratio)
    if not tail < Q(1,10**94)*(max(abs(real),abs(imag))-tail):
        raise ValueError("INSUFFICIENT_GF2_ENCLOSURE_PRECISION")
    return real,imag,tail


def verify_source(path):
    digest = sha256(path.read_bytes()).hexdigest()
    if digest != SOURCE_SHA:
        raise ValueError("GF2_TAYLOR_WITNESS_SOURCE_DRIFT")
    return digest


def compare(row, digits, source_sha):
    """Conservative rational bound for the existing common-scale guard.

    For every point in the enclosure, upper error / lower scale bounds
    the existing relative witness. A non-pass can be provably wrong or
    merely inconclusive, but neither permits a numerical pass.
    """
    if source_sha != SOURCE_SHA or digits not in (30,100):
        raise ValueError("INVALID_GF2_WITNESS_SOURCE_OR_PRECISION")
    if row.get("status") != "COMPLETED":
        raise ValueError("GF2_WITNESS_REQUIRES_COMPLETED_OUTPUT")
    re,im,tail = enclosure()
    cr,ci = Q(row["real"]),Q(row["imag"])
    center_error = max(abs(re-cr),abs(im-ci))
    scale_lower = max(Q(1),abs(cr),abs(ci),abs(re)-tail,abs(im)-tail)
    scale_upper = max(Q(1),abs(cr),abs(ci),abs(re)+tail,abs(im)+tail)
    upper = (center_error+tail)/scale_lower
    lower = max(Q(0),center_error-tail)/scale_upper
    guard = Q(1,10**(digits-6))
    sat_ok = row.get("sat") is True
    passed = sat_ok and upper <= guard
    status = ("PASS" if passed else "FAIL_SAT" if not sat_ok else
              "FAIL_OUTSIDE_GUARD" if lower > guard else "INCONCLUSIVE_GUARD_BOUNDARY")
    return {"pass":passed,"status":status,"source_sha256":source_sha,
            "reference_kind":"EXACT_GMP_TAYLOR_COEFFICIENTS_RATIONAL_ENCLOSURE",
            "terms":len(COEFFICIENTS),"sat_expected":True,
            "reference_real":str(re),"reference_imag":str(im),
            "absolute_complex_tail_bound":str(tail),
            "absolute_real_center_delta":str(abs(re-cr)),
            "absolute_imag_center_delta":str(abs(im-ci)),
            "relative_lower_bound":str(lower),"relative_upper_bound":str(upper),
            "relative_guard":f"1e-{digits-6}"}


def self_test():
    re,im,tail = enclosure()
    assert tail > 0
    row = {"status":"COMPLETED","sat":True,"real":str(re),"imag":str(im)}
    for digits in (30,100):
        guard = Q(1,10**(digits-6))
        assert compare(row,digits,SOURCE_SHA)["pass"]
        near = dict(row,real=str(re*(1+guard/2)))
        far = dict(row,real=str(re*(1+4*guard)))
        assert compare(near,digits,SOURCE_SHA)["pass"]
        assert compare(far,digits,SOURCE_SHA)["status"] == "FAIL_OUTSIDE_GUARD"
        assert compare(dict(row,sat=False),digits,SOURCE_SHA)["status"] == "FAIL_SAT"
        assert compare(dict(row,imag=str(im+abs(re)*4*guard)),digits,SOURCE_SHA)["status"] == "FAIL_OUTSIDE_GUARD"
        # Values exactly at the reference-center guard must not be
        # accepted merely because the center difference is small.
        boundary = dict(row,real=str(re*(1-guard)))
        assert compare(boundary,digits,SOURCE_SHA)["status"] == "INCONCLUSIVE_GUARD_BOUNDARY"
    for bad_row,digits,digest in ((row,100,"0"*64),(row,31,SOURCE_SHA),
                                 (dict(row,status="CENSORED_TIME"),100,SOURCE_SHA)):
        try:
            compare(bad_row,digits,digest)
        except ValueError:
            pass
        else:
            raise AssertionError("unexpected witness admission")
    print("GF2_EXACT_TAYLOR_WITNESS_SELFTEST_PASS",flush=True)


if __name__ == "__main__":
    self_test()
