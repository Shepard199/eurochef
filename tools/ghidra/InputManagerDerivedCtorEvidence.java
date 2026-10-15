import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.*;
import ghidra.program.model.symbol.*;
import java.util.*;
public class InputManagerDerivedCtorEvidence extends GhidraScript {
 DecompInterface d=new DecompInterface();
 void dec(Function f)throws Exception{if(f==null)return;println("\n=== "+f.getName()+" @ "+f.getEntryPoint()+" ===");DecompileResults r=d.decompileFunction(f,60,monitor);if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());}
 void refs(long x)throws Exception{println(String.format("\n=== REFS TO 0x%08X ===",x));LinkedHashSet<Function> fs=new LinkedHashSet<>();ReferenceIterator it=currentProgram.getReferenceManager().getReferencesTo(toAddr(x));while(it.hasNext()){Reference r=it.next();Function f=getFunctionContaining(r.getFromAddress());println(r.getReferenceType()+" "+r.getFromAddress()+" "+(f==null?"<none>":f.getName()+"@"+f.getEntryPoint()));if(f!=null)fs.add(f);}for(Function f:fs)dec(f);}
 public void run()throws Exception{d.openProgram(currentProgram);refs(0x004f89d3L);refs(0x005f3ba0L);d.dispose();}
}