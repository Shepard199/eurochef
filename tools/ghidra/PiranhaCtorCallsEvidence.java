import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.*;
import ghidra.program.model.symbol.Reference;
public class PiranhaCtorCallsEvidence extends GhidraScript {
 @Override public void run() throws Exception {
  Address s=toAddr(0x00467580L); disassemble(s);
  InstructionIterator it=currentProgram.getListing().getInstructions(s,true);
  int n=0;
  while(it.hasNext()&&n<140){
   Instruction ins=it.next();
   if(ins.getAddress().getOffset()>=0x00467600L) break;
   if(ins.getFlowType().isCall()||ins.toString().contains("0x654")||ins.toString().contains("0x655"))
     println(ins.getAddress()+"  "+ins);
   for(Reference r:ins.getReferencesFrom()) if(r.getReferenceType().isCall()) println("  CALLREF -> "+r.getToAddress());
   n++;
  }
 }
}