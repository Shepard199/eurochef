import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
public class PiranhaUpdateEvidence2 extends GhidraScript {
  @Override public void run()throws Exception{
    DecompInterface d=new DecompInterface();d.openProgram(currentProgram);
    Function f=getFunctionAt(toAddr(0x004676F0L));
    if(f==null){disassemble(toAddr(0x004676F0L));try{f=createFunction(toAddr(0x004676F0L),"evidence_4676f0");}catch(Exception ignored){}}
    if(f==null)f=getFunctionContaining(toAddr(0x004676F0L));
    DecompileResults r=d.decompileFunction(f,90,monitor);
    if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());
    d.dispose();
  }
}